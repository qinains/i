import {recoverHistory} from './state/recovery.js';
import {normalizeEndpoint} from './transport.js';
import {validateSession} from './state/records.js';
import {isTemporaryWorkspace} from './workspaces.js';
import {initializeTitle} from './titles.js';
export function connectionIdentity(config){return JSON.stringify([normalizeEndpoint(config.base_url,config.protocol),config.model,config.protocol,config.transport]);}
export class SessionController {
    constructor(Agent,store,hooks){this.Agent=Agent;this.store=store;this.hooks=hooks;this.records=new Map();this.agents=new Map();this.agentStarts=new Map();this.queues=new Map();this.timers=new Map();this.failed=new Map();}
    async save(record){
        if(isTemporaryWorkspace(record.workspaceId)){record.updatedAt=Date.now();this.hooks.saving?.(record.id,'Temporary — not saved');return;}
        this.hooks.saving?.(record.id,'Saving…');
        const previous=this.queues.get(record.id) ?? Promise.resolve();
        const next=previous.catch(()=>{}).then(async()=>{
            if(this.failed.has(record.id))throw this.failed.get(record.id);
            record.revision=await this.store.commitSession(structuredClone(record),record.revision);
        });
        this.queues.set(record.id,next);
        try{await next;this.hooks.saving?.(record.id,this.queues.get(record.id)===next&&!this.timers.has(record.id)?'Saved':'Saving…');}
        catch(error){this.failed.set(record.id,error);this.hooks.saving?.(record.id,'Save failed');this.hooks.error?.(error);throw error;}
    }
    later(record){
        this.hooks.saving?.(record.id,'Saving…');clearTimeout(this.timers.get(record.id));
        this.timers.set(record.id,setTimeout(()=>{this.timers.delete(record.id);this.save(record).catch(()=>{});},150));
    }
    async flush(id){
        if(id){const record=this.records.get(id);if(this.timers.has(id)){clearTimeout(this.timers.get(id));this.timers.delete(id);await this.save(record);}await this.queues.get(id);}
        else await Promise.all([...this.records.keys()].map(key=>this.flush(key)));
    }
    async create(workspaceId,name,connectionId=''){
        const record={id:crypto.randomUUID(),workspaceId,name,titleSource:'default',connectionId,connectionIdentity:'',mode:'ask',history:[],transcript:[],draft:'',status:'idle',revision:0,journal:null,updatedAt:Date.now()};
        await this.save(record);this.records.set(record.id,record);return record;
    }
    async restore(records,sanitize=value=>value){
        // Limit retained attachment bytes across all opened saved sessions.
        let imageBudget=128*1024*1024;
        for(let record of records){
            let stage='metadata';
            try{
                validateSession(record);
                stage='attachments';
                const prepared=await this.store.prepareSession?.(record,imageBudget);
                if(prepared){record=prepared.record;imageBudget-=prepared.bytes;}
                if(record.pendingMessages?.length)record.queuePaused=true;
                stage='recovery';const recovered=recoverHistory(record.history,record.journal);
                stage='history';
                // create() persists before lazy agent initialization supplies the system message.
                if(recovered.length)this.Agent.validate_history(JSON.stringify(recovered));
                stage='title';
                const migrate=!record.titleSource;initializeTitle(record,text=>sanitize(text,record));
                stage='checkpoint';
                if(record.status==='running'||record.status==='approval'||record.journal){
                    record.history=recovered;record.journal=null;record.status='interrupted';
                    for(const line of record.transcript)if(line.pending){line.text+=' [interrupted]';delete line.pending;}
                    record.transcript.push({type:'system',text:'Task interrupted by reload. No request, approval or tool was automatically repeated.'});await this.save(record);
                }else if(migrate)await this.save(record);
                this.records.set(record.id,record);
            }catch(error){this.records.delete(record.id);this.hooks.error?.(new Error(`A saved session is invalid or could not be recovered. It was retained in storage, not opened. (stage: ${stage})`));}
        }
    }
    async agent(record){
        if(this.agents.has(record.id))return this.agents.get(record.id);
        if(this.agentStarts.has(record.id))return this.agentStarts.get(record.id);
        const pending=this.initializeAgent(record);this.agentStarts.set(record.id,pending);
        try{return await pending;}finally{if(this.agentStarts.get(record.id)===pending)this.agentStarts.delete(record.id);}
    }
    async initializeAgent(record){
        const agent=await this.Agent.create_workspace(record.workspaceId,event=>this.hooks.event(record.id,event),description=>this.hooks.approve(record.id,description),archive=>this.hooks.download(record.id,archive),async snapshot=>{
            record.history=snapshot.history;record.journal=snapshot.journal;if(!record.journal)this.hooks.committed?.(record);await this.save(record);
        });
        try{
            if(record.history.length)agent.import_history(JSON.stringify(record.history));
            else{record.history=JSON.parse(agent.export_history());await this.save(record);}
            this.agents.set(record.id,agent);return agent;
        }catch(error){agent.free?.();throw error;}
    }
    async configure(record,config,connectionId=''){
        const identity=connectionIdentity(config);
        if(record.history.some(m=>m.role==='user') && record.connectionId!==connectionId)throw new Error('This conversation belongs to another or deleted connection. Use Settings to explicitly attach the selected connection, or create a new conversation.');
        if(record.connectionIdentity && record.connectionIdentity!==identity && record.history.some(m=>m.role==='user'))throw new Error('This conversation belongs to another connection/model. Select its original connection or create a new conversation.');
        const agent=await this.agent(record);agent.configure(JSON.stringify(config));
        if(record.history.length)agent.import_history(JSON.stringify(record.history));
        record.connectionIdentity=identity;return agent;
    }
    cancelAll(){for(const agent of this.agents.values())agent.cancel();}
}
