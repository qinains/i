const KEY='i_harness_theme';
const valid=value=>['auto','light','dark'].includes(value);
export function createTheme({root,storage,media,meta,window:host}){
    let mode='auto';const listeners=new Set();
    try{const saved=storage.getItem(KEY);if(valid(saved))mode=saved;}catch{}
    function apply(){
        const resolved=mode==='auto'?(media.matches?'dark':'light'):mode;
        root.dataset.theme=resolved;root.dataset.appearance=mode;root.style.colorScheme=resolved;
        if(meta)meta.content=resolved==='dark'?'#101318':'#f3f5f8';
        for(const listener of listeners)listener(mode);
    }
    media.addEventListener('change',()=>{if(mode==='auto')apply();});
    host?.addEventListener('storage',event=>{if(event.key===KEY||event.key===null){mode=valid(event.newValue)?event.newValue:'auto';apply();}});
    apply();
    return {
        get mode(){return mode;},
        subscribe(listener){listeners.add(listener);listener(mode);return ()=>listeners.delete(listener);},
        setMode(value){
            if(!valid(value))throw new Error('Invalid theme.');
            mode=value;apply();try{storage.setItem(KEY,value);return true;}catch{return false;}
        },
    };
}
let storage;
if(typeof document!=='undefined')try{storage=globalThis.localStorage;}catch{}
export const theme=typeof document==='undefined'?null:createTheme({root:document.documentElement,storage,media:matchMedia('(prefers-color-scheme: dark)'),meta:document.querySelector('meta[name=theme-color]'),window:globalThis});
