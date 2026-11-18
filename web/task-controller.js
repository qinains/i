import {withWorkspaceLock} from './state/store.js';
import {connectionIdentity} from './sessions.js';
import {composeDraft,imageDataUrls} from './composer.js';
import {enqueueMessage,removeMessage} from './pending.js';
import {runPluginTask} from './plugins/agent.js';

export class TaskController {
    #current;
    #approval;
    #draining=false;
    #queueSending;
    #view;
    #lock;

    constructor(view,lock=withWorkspaceLock){this.#view=view;this.#lock=lock;}
    get runningId(){return this.#current?.record.id;}
    get pendingApproval(){const {id,description}=this.#approval??{};return this.#approval&&{id,description};}
    get queueSending(){return this.#queueSending;}
    composerBlocked(id){const task=this.#current;return !!task&&task.record.id===id&&(task.phase==='preparing'||!task.message);}

    async approve(id,description,{force=false}={}){
        const task=this.#current;
        if(!task||task.record.id!==id||task.signal.aborted||task.pluginAbort?.signal.aborted||task.phase==='approval')return false;
        const {record}=task,sessions=this.#view.sessions();
        if(record.mode==='auto'&&!force){await sessions.save(record);return !task.signal.aborted&&!task.pluginAbort?.signal.aborted;}
        record.status='approval';task.phase='approval';
        await sessions.save(record);
        if(task.signal.aborted||task.pluginAbort?.signal.aborted)return false;
        return new Promise(resolve=>{this.#approval={id,description,resolve};this.#view.changed();});
    }
    settleApproval(allowed,id){
        const approval=this.#approval;
        if(!approval||(id!==undefined&&id!==approval.id))return;
        this.#approval=null;
        const task=this.#current;
        if(task?.record.id===approval.id){task.record.status='running';task.phase='running';}
        approval.resolve(allowed&&!task?.signal.aborted);this.#view.changed();
    }
    adapterCancelled(id){
        const task=this.#current;
        if(task?.record.id!==id||!task?.pluginAbort)return;
        // Adapter retirement also emits this event; only cancel the current plugin invocation.
        task.pluginAbort.abort();this.settleApproval(false,id);
    }
    cancel(id,{pause=false}={}){
        const task=this.#current;
        if(!task||task.record.id!==id||task.signal.aborted)return;
        task.abort.abort();
        task.agent?.cancel();
        if(pause){task.record.queuePaused=true;this.#view.sessions().later(task.record);}
        this.settleApproval(false,id);this.#view.changed();
    }

    async run(record,fn,{readOnly=false,runQueue=false,message=false,preparing=false,historyOnly=false}={}){
        const view=this.#view,sessions=view.sessions();
        if(this.#current||view.blocked())throw new Error('Finish or stop the current task first.');
        if(sessions.conflicts.has(record.id)&&!readOnly)throw new Error('Saving failed. Download a ZIP backup, then reload before running more tools.');
        const abort=new AbortController(),task={record,abort,signal:abort.signal,message,phase:preparing?'preparing':'running'};
        this.#current=task;record.status='running';view.changed();let succeeded=false;
        try{
            if(!readOnly)await sessions.save(record);
            task.signal.throwIfAborted();
            const execute=async()=>{
                if(!readOnly)await sessions.flush(record.id);
                task.signal.throwIfAborted();
                task.agent=await sessions.agent(record);
                task.signal.throwIfAborted();
                await fn(task.agent,task.signal);
            };
            if(historyOnly)await execute();else await this.#lock(record.workspaceId,execute);
            succeeded=!task.signal.aborted;
        }catch(error){record.queuePaused=true;abort.abort();this.#taskError(record,error);}
        finally{
            this.settleApproval(false,record.id);task.phase='finishing';record.status='idle';
            if(!readOnly)try{await sessions.save(record);}catch(error){succeeded=false;record.queuePaused=true;view.report(error);}
            if(!historyOnly&&!task.signal.aborted)try{await view.refresh(task.signal);}catch(error){view.report(error);}
            const backgroundRefresh=!historyOnly&&task.signal.aborted;
            abort.abort();this.#current=null;view.changed();view.focus(record);
            if(backgroundRefresh)view.refresh().catch(view.report);
            if(runQueue&&succeeded&&!record.queuePaused)setTimeout(()=>this.drain(record).catch(view.report),0);
        }
        return succeeded;
    }

    async enqueue(record,text){
        const view=this.#view,{id,config}=view.connection();
        enqueueMessage(record,text,{connectionId:id,connectionIdentity:connectionIdentity(config)});
        this.#clearDraft(record);view.changed();
        try{await view.sessions().save(record);}catch(error){record.queuePaused=true;throw error;}finally{view.changed();}
    }
    async submit(record,text){
        if(this.#view.blocked()||this.composerBlocked(record.id))throw new Error('Wait for the active operation.');
        if(this.#current&&this.runningId!==record.id)throw new Error('Another conversation is running.');
        if(this.#current||record.pendingMessages?.length){await this.enqueue(record,text);return true;}
        if(/^[\/!]/u.test(text.trimStart()))return false;
        await this.send(record,text,structuredClone(record.composerBlocks??[]));return true;
    }
    async resume(record){
        const sessions=this.#view.sessions();
        if(sessions.conflicts.has(record.id)||record.pendingMessages?.some(entry=>entry.editDraft!=null))return;
        record.queuePaused=false;
        try{await sessions.save(record);}catch(error){record.queuePaused=true;throw error;}finally{this.#view.changed();}
        if(!this.#current)await this.drain(record);
    }
    async pause(record){
        record.queuePaused=true;
        try{await this.#view.sessions().save(record);}finally{this.#view.changed();}
    }
    async drain(record){
        const view=this.#view,sessions=view.sessions();
        if(this.#draining||this.#current||view.blocked())return;
        this.#draining=true;
        try{
            while(!record.queuePaused&&record.pendingMessages?.length&&!this.#current&&!view.blocked()){
                const entry=record.pendingMessages[0];
                if(entry.editDraft!=null){record.queuePaused=true;await sessions.save(record);break;}
                this.#queueSending={id:record.id,messageId:entry.id};view.changed();
                let succeeded=false;
                try{succeeded=await this.send(record,entry.visible,entry.blocks,entry);}
                catch(error){record.queuePaused=true;this.#taskError(record,error);}
                finally{this.#queueSending=null;}
                if(!succeeded){record.queuePaused=true;await sessions.save(record);break;}
            }
        }finally{this.#draining=false;this.#queueSending=null;view.changed();}
    }

    #clearDraft(record){record.draft='';record.composerBlocks=[];this.#view.draftCleared(record);}
    #taskError(record,error){
        const view=this.#view,message=String(error?.message??error);
        view.append(record,`Error: ${view.translate?.(message)??message}`,'error');
        if(error?.code==='invalid_api_url')view.report(error);
    }
    async send(record,visible,blocks,entry=null){
        const view=this.#view,sessions=view.sessions(),composed=composeDraft({composerBlocks:blocks},visible);
        if(!view.online())throw new Error('Offline: the unsent message and attachments are retained.');
        view.validateConnection?.();
        if(!entry)view.title(record,composed.text||'Image');
        return this.run(record,async(_agent,signal)=>{
            const {id,config}=view.connection();
            if(entry&&(entry.connectionId!==id||entry.connectionIdentity!==connectionIdentity(config)))throw new Error('Queued message belongs to another connection/model. Select its original connection before continuing.');
            const agent=await sessions.configure(record,config,id);this.#current.agent=agent;record.connectionId=id;
            const images=await imageDataUrls(composed.images);
            let manager,store;
            try{({manager,store}=await view.plugins());}catch{view.append(record,'Plugin storage is unavailable; installed plugin tools are disabled.','error');}
            signal.throwIfAborted();
            const {draft,composerBlocks}=record;
            if(entry)removeMessage(record,entry.id);
            else{record.queuePaused=false;record.draft='';record.composerBlocks=[];}
            const line={type:'user',text:view.redact(`> ${visible}`)};record.transcript.push(line);
            try{await sessions.save(record);}catch(error){
                record.transcript=record.transcript.filter(item=>item!==line);
                if(entry)(record.pendingMessages??=[]).unshift(entry);
                else{record.draft=draft;record.composerBlocks=composerBlocks;}
                throw error;
            }
            this.#current.phase='running';
            if(!entry)view.draftCleared(record);
            view.transcriptChanged(record);if(entry)view.title(record,composed.text||'Image');view.changed();signal.throwIfAborted();
            const task=this.#current,pluginAbort=new AbortController();task.pluginAbort=pluginAbort;
            try{await runPluginTask({agent,manager,store,record,text:composed.text,images,signal:AbortSignal.any([signal,pluginAbort.signal]),approve:(description,options)=>this.approve(record.id,description,options),beforeEffect:()=>sessions.flush(record.id),onUnavailable:id=>view.append(record,`Plugin unavailable: ${id}`,'error')});}
            finally{pluginAbort.abort();task.pluginAbort=null;}
        },{runQueue:true,message:true,preparing:true});
    }
}
