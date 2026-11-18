import {recoverHistory} from './state/recovery.js';
import {normalizeEndpoint} from './transport.js';
import {validateSession} from './state/records.js';
import {isTemporaryWorkspace} from './workspaces.js';
import {initializeTitle} from './titles.js';
import {SessionScope,WorkspaceService} from './session-scope.js';
export function connectionIdentity(config){return JSON.stringify([normalizeEndpoint(config.base_url,config.protocol),config.model,config.protocol,config.transport]);}
export class SessionController {
    constructor(Agent,store,hooks,context){
        if(!context)throw new Error('SessionController requires an application context.');
        this.Agent=Agent;this.store=store;this.hooks=hooks;this.context=context;
        this.records=new Map();this.agents=new Map();this.agentStarts=new Map();this.scopes=new Map();this.workspaces=new Map();
        this.queues=new Map();this.timers=new Map();this.failed=new Map();this.closing=false;this.closingSessions=new Set();this.closes=new Map();this.closeResults=new Map();this.shutdownPromise=null;
        context.effect(()=>()=>{this.closing=true;this.cancelAll();});
    }
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
        if(this.closing)throw new Error('Session controller is closing.');
        const record={id:crypto.randomUUID(),workspaceId,name,titleSource:'default',connectionId,connectionIdentity:'',mode:'ask',history:[],transcript:[],draft:'',status:'idle',revision:0,journal:null,updatedAt:Date.now()};
        await this.save(record);this.records.set(record.id,record);return record;
    }
    async restore(records,sanitize=value=>value){
        for(const record of records){
            let stage='metadata';
            try{
                validateSession(record);
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
        if(this.closing||this.closingSessions.has(record.id))throw new Error('Session is closing.');
        if(this.agents.has(record.id))return this.agents.get(record.id);
        if(this.agentStarts.has(record.id))return this.agentStarts.get(record.id);
        const pending=this.initializeAgent(record);this.agentStarts.set(record.id,pending);
        try{return await pending;}finally{if(this.agentStarts.get(record.id)===pending)this.agentStarts.delete(record.id);}
    }
    async initializeAgent(record){
        const agent=await this.scope(record).ready();
        if(this.closing||this.closingSessions.has(record.id))throw new Error('Session is closing.');
        this.agents.set(record.id,agent);return agent;
    }
    scope(record){
        if(this.scopes.has(record.id))return this.scopes.get(record.id);
        if(this.closing||this.closingSessions.has(record.id))throw new Error('Session is closing.');
        if(!this.workspaces.has(record.workspaceId)){
            const view=this.context.isolate('workspace',Symbol(record.workspaceId));
            this.workspaces.set(record.workspaceId,view.use(WorkspaceService,{id:record.workspaceId,Agent:this.Agent}));
        }
        const parent=this.workspaces.get(record.workspaceId).ctx;
        const scope=new SessionScope(parent,{record,hooks:this.hooks,save:record=>this.save(record),flush:id=>this.flush(id),
            isClosing:()=>this.closing||this.closingSessions.has(record.id),onReleased:()=>this.agents.delete(record.id)});
        this.scopes.set(record.id,scope);return scope;
    }
    async configure(record,config,connectionId=''){
        const identity=connectionIdentity(config);
        if(record.history.some(m=>m.role==='user') && record.connectionId!==connectionId)throw new Error('This conversation belongs to another or deleted connection. Use Settings to explicitly attach the selected connection, or create a new conversation.');
        if(record.connectionIdentity && record.connectionIdentity!==identity && record.history.some(m=>m.role==='user'))throw new Error('This conversation belongs to another connection/model. Select its original connection or create a new conversation.');
        const agent=await this.scope(record).configure(config);
        this.agents.set(record.id,agent);
        record.connectionIdentity=identity;return agent;
    }
    cancelAll(){for(const scope of this.scopes.values())scope.cancel();}
    close(id){
        if(this.closes.has(id))return this.closes.get(id);
        this.closingSessions.add(id);
        const closing=(async()=>{
            const errors=[];
            const scope=this.scopes.get(id);
            if(scope)return scope.close();
            try{await this.flush(id);}catch(error){errors.push(String(error?.message??error));}
            return {closed:true,errors,retained:[]};
        })().then(report=>{this.closeResults.set(id,report);return report;});
        this.closes.set(id,closing);return closing;
    }
    shutdown({timeoutMs=10000}={}){
        if(this.shutdownPromise)return this.shutdownPromise;
        if(!Number.isSafeInteger(timeoutMs)||timeoutMs<1)throw new Error('Invalid shutdown timeout.');
        this.closing=true;
        const ids=new Set([...this.records.keys(),...this.scopes.keys(),...this.agents.keys(),...this.agentStarts.keys(),...this.timers.keys(),...this.queues.keys()]);
        const drained=Promise.all([...ids].map(id=>this.close(id))).then(reports=>({closed:reports.every(report=>report.closed),errors:reports.flatMap(report=>report.errors),retained:reports.flatMap(report=>report.retained)}));
        this.shutdownPromise=new Promise(resolve=>{
            const timer=setTimeout(()=>resolve({closed:false,errors:['Session shutdown timed out; in-use adapters were retained.'],retained:[...ids].filter(id=>!this.closeResults.get(id)?.closed)}),timeoutMs);
            drained.then(report=>{clearTimeout(timer);resolve(report);},error=>{clearTimeout(timer);resolve({closed:false,errors:[String(error?.message??error)],retained:[...ids].filter(id=>!this.closeResults.get(id)?.closed)});});
        });
        return this.shutdownPromise;
    }
}
