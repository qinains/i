const LIMIT=32*1024*1024;
const missing=()=>new DOMException('Entry not found','NotFoundError');
const mismatch=()=>new DOMException('Entry has a different kind','TypeMismatchError');
export class MemoryDirectory {
    constructor(budget={bytes:0}){this.kind='directory';this.entriesMap=new Map();this.budget=budget;}
    async getDirectoryHandle(name,{create=false}={}){
        let value=this.entriesMap.get(name);
        if(!value&&create){value=new MemoryDirectory(this.budget);this.entriesMap.set(name,value);}
        if(!value)throw missing();if(value.kind!=='directory')throw mismatch();return value;
    }
    async getFileHandle(name,{create=false}={}){
        let value=this.entriesMap.get(name);
        if(!value&&create){value={kind:'file',bytes:new Uint8Array()};this.entriesMap.set(name,value);}
        if(!value)throw missing();if(value.kind!=='file')throw mismatch();
        const budget=this.budget;
        return {kind:'file',getFile:async()=>{const bytes=value.bytes.slice();return {size:bytes.length,arrayBuffer:async()=>bytes.buffer};},createWritable:async()=>{
            let pending,closed=false;
            return {write:async bytes=>{if(closed)throw new Error('Writer is closed');pending=new Uint8Array(bytes).slice();},
                close:async()=>{if(closed)throw new Error('Writer is closed');closed=true;if(!pending)return;
                    const size=budget.bytes-value.bytes.length+pending.length;
                    if(size>LIMIT)throw new DOMException('Temporary storage exceeds 32 MiB','QuotaExceededError');
                    budget.bytes=size;value.bytes=pending;},abort:async()=>{closed=true;pending=null;}};
        }};
    }
    async *keys(){yield* this.entriesMap.keys();}
    async *entries(){yield* this.entriesMap.entries();}
    async removeEntry(name){
        const value=this.entriesMap.get(name);if(!value)throw missing();
        if(value.kind==='directory'&&value.entriesMap.size)throw new DOMException('Directory is not empty','InvalidModificationError');
        if(value.kind==='file')this.budget.bytes-=value.bytes.length;
        this.entriesMap.delete(name);
    }
    clear(){
        const stack=[this];let removed=0;
        while(stack.length)for(const entry of stack.pop().entriesMap.values()){
            if(entry.kind==='directory')stack.push(entry);else removed+=entry.bytes.length;
        }
        this.entriesMap.clear();this.budget.bytes-=removed;
    }
}
