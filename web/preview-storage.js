// Serialized into the opaque iframe; never bridge to the workbench's storage.
export function installPreviewStorage(target=window,{maxBytes=1024*1024}={}){
    const budget={bytes:0};
    function create(){
        const values=new Map();
        const bytes=(key,value)=>2*(key.length+value.length);
        const methods={
            getItem(key){return values.get(String(key))??null;},
            setItem(key,value){
                key=String(key);value=String(value);
                const previous=values.get(key),delta=bytes(key,value)-(previous===undefined?0:bytes(key,previous));
                if(budget.bytes+delta>maxBytes)throw new DOMException('Preview storage exceeds its temporary quota.','QuotaExceededError');
                values.set(key,value);budget.bytes+=delta;
            },
            removeItem(key){key=String(key);const previous=values.get(key);if(previous!==undefined){budget.bytes-=bytes(key,previous);values.delete(key);}},
            clear(){for(const [key,value]of values)budget.bytes-=bytes(key,value);values.clear();},
            key(index){return [...values.keys()][Number(index)>>>0]??null;},
        };
        return new Proxy(Object.create(null),{
            get(_target,key){
                if(key===Symbol.toStringTag)return 'Storage';
                if(key==='length')return values.size;
                if(Object.hasOwn(methods,key))return methods[key];
                return typeof key==='string'?values.get(key):undefined;
            },
            set(_target,key,value){if(typeof key!=='string')return false;methods.setItem(key,value);return true;},
            deleteProperty(_target,key){if(typeof key==='string')methods.removeItem(key);return true;},
            has(_target,key){return key==='length'||Object.hasOwn(methods,key)||values.has(key);},
            ownKeys(){return [...values.keys()];},
            getOwnPropertyDescriptor(_target,key){if(values.has(key))return {value:values.get(key),writable:true,enumerable:true,configurable:true};},
        });
    }
    for(const key of ['localStorage','sessionStorage'])Object.defineProperty(target,key,{value:create(),configurable:true,enumerable:true});
}
