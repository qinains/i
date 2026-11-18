import {Service} from './plugins/vendor/cordis.js';

export class WorkspaceService extends Service {
    constructor(ctx,{id,Agent}){super(ctx,'workspace');this._state={id,Agent};}
    open(...callbacks){return this._state.Agent.create_workspace(this._state.id,...callbacks);}
}

const OperationComponent={
    name:'browser-operation',inject:['adapter'],
    apply(ctx,{state,method,args,completion,cancellable}){
        completion.started=true;
        let operation;
        try{
            if(state.scope.isClosing()||state.model.revision!==state.scope.revision)throw new Error('Session adapter is closing or retired.');
            operation=Promise.resolve(state.agent[method](...args));
        }catch(error){operation=Promise.reject(error);}
        ctx.effect(()=>()=>{if(cancellable)state.agent.cancel();return operation.catch(()=>{});});
        operation.then(completion.resolve,completion.reject);
    },
};

class AdapterService extends Service {
    constructor(ctx,state){super(ctx,'adapter');this._state=state;}
    _assert(){
        const {scope,model,agent}=this._state;
        if(!agent||scope.isClosing()||model.revision!==scope.revision)throw new Error('Session adapter is closing or retired.');
    }
    _invoke(method,args){this._assert();return this._state.agent[method](...args);}
    async _operation(method,args,cancellable=false){
        this._assert();
        const completion=Promise.withResolvers();completion.promise.catch(()=>{});
        const fiber=this.ctx.use(OperationComponent,{state:this._state,method,args,completion,cancellable});
        try{
            await fiber;
            if(!completion.started)throw new Error('Session adapter is closing or retired.');
            return await completion.promise;
        }finally{await fiber.dispose();}
    }
    run(...args){return this._operation('run',args,true);}
    run_message(...args){return this._operation('run_message',args,true);}
    cancel(){this._state.agent?.cancel();}
    export_history(){return this._state.agent?this._state.agent.export_history():JSON.stringify(this._state.scope.record.history);}
    forget_credentials(){
        this._invoke('forget_credentials',[]);
        this._state.model.config=null;this._state.scope.configuration=null;
    }
}
for(const method of ['import_history','new_session','set_plugin_tools']){
    Object.defineProperty(AdapterService.prototype,method,{value(...args){return this._invoke(method,args);}});
}
for(const method of ['upload','list_files','export_zip']){
    Object.defineProperty(AdapterService.prototype,method,{value(...args){return this._operation(method,args);}});
}

const SessionComponent={
    name:'browser-session',inject:['workspace','sessionRecord','model'],
    async apply(ctx,{scope}){
        const record=ctx.sessionRecord,model=ctx.model;
        let state;
        await ctx.effect(async()=>{
            const agent=await ctx.workspace.open(
                event=>scope.hooks.event(record.id,event),
                description=>scope.hooks.approve(record.id,description),
                archive=>scope.hooks.download(record.id,archive),
                async snapshot=>{
                    record.history=snapshot.history;record.journal=snapshot.journal;
                    if(!record.journal)scope.hooks.committed?.(record);
                    await scope.save(record);
                },
            );
            state={agent,model,scope};scope.owned.add(state);
            return async()=>{
                try{
                    await agent.close();
                    try{await scope.flush(record.id);}catch(error){scope.errors.push(String(error?.message??error));}
                    agent.free();state.agent=null;model.config=null;scope.owned.delete(state);
                    if(scope.state===state){scope.state=null;scope.onReleased();}
                }catch(error){scope.errors.push(String(error?.message??error));}
            };
        });
        if(scope.isClosing()||model.revision!==scope.revision||ctx.fiber.uid===null)return;
        if(model.config)state.agent.configure(JSON.stringify(model.config));
        if(record.history.length)state.agent.import_history(JSON.stringify(record.history));
        else{record.history=JSON.parse(state.agent.export_history());await scope.save(record);}
        if(scope.isClosing()||model.revision!==scope.revision||ctx.fiber.uid===null)return;
        new AdapterService(ctx,state);scope.state=state;scope.context=ctx;
    },
};

export class SessionScope {
    constructor(parent,{record,hooks,save,flush,isClosing,onReleased}){
        Object.assign(this,{record,hooks,save,flush,onReleased});
        this.parentClosing=isClosing;this.closed=false;this.revision=0;this.configuration=null;
        this.owned=new Set();this.errors=[];this.state=null;
        this.view=parent.isolate('sessionRecord',Symbol(record.id)).isolate('model',Symbol(record.id)).isolate('adapter',Symbol(record.id));
        this.recordLease=this.view.provide('sessionRecord',record);
        this.modelLease=this.view.provide('model',{revision:0,config:null});
        this.fiber=this.view.use(SessionComponent,{scope:this});
    }
    isClosing(){return this.closed||this.parentClosing();}
    async ready(){
        await this.view.fiber.await();
        await this.fiber;
        if(this.isClosing()||this.fiber.uid===null)throw new Error('Session is closing.');
        if(!this.state)throw new Error('Session adapter is not ready.');
        return this.context.adapter;
    }
    async configure(config){
        if(this.isClosing())throw new Error('Session is closing.');
        const serialized=JSON.stringify(config);
        let revision=this.revision;
        if(serialized!==this.configuration||this.fiber.state===3){
            this.cancel();revision=++this.revision;
            const retiring=this.modelLease();
            this.modelLease=this.view.provide('model',{revision:this.revision,config:structuredClone(config)});
            this.configuration=serialized;
            await this.fiber.update({scope:this});await retiring;
        }
        const agent=await this.ready();
        if(revision!==this.revision)throw new Error('Session configuration was superseded.');
        return agent;
    }
    cancel(){for(const state of this.owned)state.agent?.cancel();}
    close(){
        if(this.closing)return this.closing;
        this.closed=true;this.cancel();
        this.closing=(async()=>{
            await this.fiber.dispose();await this.modelLease();await this.recordLease();
            try{await this.flush(this.record.id);}catch(error){this.errors.push(String(error?.message??error));}
            return {closed:this.owned.size===0,errors:[...this.errors],retained:this.owned.size?[this.record.id]:[]};
        })();
        return this.closing;
    }
}
