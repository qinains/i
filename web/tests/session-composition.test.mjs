import assert from 'node:assert/strict';
import {test} from 'node:test';
import {Context} from '../plugins/vendor/cordis.js';
import {SessionController} from '../sessions.js';

const record=id=>({id,workspaceId:'default',history:[{role:'system',content:'keep'}],connectionId:'a',connectionIdentity:''});
const config=model=>({base_url:'https://example.invalid/v1',model,protocol:'openai',transport:'direct'});

test('upstream root owns the actual session adapter, not only controller metadata',async()=>{
    const context=new Context(),events=[];
    const Agent={create_workspace:async()=>({import_history(){},cancel(){},async close(){events.push('close');},free(){events.push('free');}})};
    const controller=new SessionController(Agent,{}, {},context);
    await controller.agent(record('a'));
    await context.fiber.dispose();
    assert.deepEqual(events,['close','free']);
    assert.equal(controller.agents.size,0);
    await assert.rejects(controller.agent(record('a')),/closing|disposed/u);
});

test('ancestor disposal drains and frees a late acquired adapter without publishing it',async()=>{
    const context=new Context(),ready=Promise.withResolvers(),entered=Promise.withResolvers(),events=[];
    const Agent={create_workspace:()=>{entered.resolve();return ready.promise;}};
    const controller=new SessionController(Agent,{}, {},context);
    const starting=controller.agent(record('late'));
    await entered.promise;
    const closing=context.fiber.dispose();
    ready.resolve({import_history(){events.push('import');},cancel(){},async close(){events.push('close');},free(){events.push('free');}});
    await assert.rejects(starting,/closing|disposed/u);
    await closing;
    assert.deepEqual(events,['close','free']);
    assert.equal(controller.agents.size,0);
});

test('model replacement cancels and drains an old run before acquiring the replacement adapter',async()=>{
    const context=new Context(),entered=Promise.withResolvers(),cancelled=Promise.withResolvers(),checkpoint=Promise.withResolvers();
    const events=[];let created=0;
    const Agent={create_workspace:async()=>{
        const id=++created;let running;
        return {import_history(){},configure(){if(running)throw new Error('busy');},
            run(){running=(async()=>{entered.resolve();await checkpoint.promise;events.push(`checkpoint:${id}`);})();return running;},
            cancel(){if(running)cancelled.resolve();},
            async close(){this.cancel();await running;events.push(`close:${id}`);},free(){events.push(`free:${id}`);}};
    }};
    const controller=new SessionController(Agent,{}, {},context),session=record('run');
    const old=await controller.configure(session,config('first'),'a');
    const running=old.run('task');await entered.promise;
    const replacing=controller.configure(session,config('second'),'a');
    replacing.catch(()=>{});
    await cancelled.promise;
    assert.equal(created,1);
    await assert.rejects(old.run('stale'),/closing|retired/u);
    checkpoint.resolve();await running;
    const next=await replacing;
    assert.notEqual(next,old);assert.equal(created,2);
    assert.deepEqual(events,['checkpoint:1','close:1','free:1']);
    await controller.shutdown();await context.fiber.dispose();
});

test('session disposal drains asynchronous workspace calls before releasing their adapter',async()=>{
    const context=new Context(),entered=Promise.withResolvers(),release=Promise.withResolvers(),events=[];
    const Agent={create_workspace:async()=>({import_history(){},cancel(){},
        async upload(){entered.resolve();await release.promise;events.push('upload');},
        async close(){events.push('close');},free(){events.push('free');}})};
    const controller=new SessionController(Agent,{},{},context),agent=await controller.agent(record('upload'));
    const upload=agent.upload('file',new Uint8Array());await entered.promise;
    const closing=controller.close('upload');
    await new Promise(setImmediate);assert.deepEqual(events,[]);
    release.resolve();await upload;await closing;await context.fiber.dispose();
    assert.deepEqual(events,['upload','close','free']);
});

test('immediate model withdrawal cannot admit a queued old run',async()=>{
    const context=new Context();let runs=0;
    const Agent={create_workspace:async()=>({import_history(){},configure(){},cancel(){},run(){runs++;},async close(){},free(){}})};
    const controller=new SessionController(Agent,{},{},context),session=record('queued');
    const old=await controller.configure(session,config('first'),'a');
    const running=old.run('queued');
    const replacing=controller.configure(session,config('second'),'a');
    await assert.rejects(running,/closing|retired/u);await replacing;
    assert.equal(runs,0);await controller.shutdown();await context.fiber.dispose();
});

test('a superseded configure caller cannot publish the latest adapter under an older identity',async()=>{
    const context=new Context(),entered=Promise.withResolvers(),release=Promise.withResolvers(),models=[];let created=0;
    const Agent={create_workspace:async()=>{
        const id=++created;
        return {import_history(){},configure(value){models.push(JSON.parse(value).model);},cancel(){},async close(){if(id===1){entered.resolve();await release.promise;}},free(){}};
    }};
    const controller=new SessionController(Agent,{},{},context),session=record('concurrent');
    await controller.configure(session,config('first'),'a');
    const second=controller.configure(session,config('second'),'a');second.catch(()=>{});
    await entered.promise;
    const third=controller.configure(session,config('third'),'a');
    release.resolve();
    await assert.rejects(second,/superseded/u);await third;
    assert.equal(models.at(-1),'third');assert.equal(session.connectionIdentity,JSON.stringify(['https://example.invalid/v1/chat/completions','third','openai','direct']));
    await controller.shutdown();await context.fiber.dispose();
});
