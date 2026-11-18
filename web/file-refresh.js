export class FileRefresh {
    constructor(){this.sequence=0;this.controller=null;}
    cancel(){this.sequence++;this.controller?.abort();this.controller=null;}
    async run(load,apply,external){
        if(external?.aborted)return false;
        this.cancel();const sequence=this.sequence,controller=new AbortController();this.controller=controller;
        const signal=external?AbortSignal.any([external,controller.signal]):controller.signal;
        try{
            signal.throwIfAborted();const value=await load(signal);
            if(signal.aborted||sequence!==this.sequence)return false;
            apply(value);return true;
        }catch(error){if(signal.aborted||sequence!==this.sequence)return false;throw error;}
        finally{if(this.controller===controller)this.controller=null;}
    }
}
