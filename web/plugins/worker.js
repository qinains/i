import init,{inspect_plugin_package,inspect_plugin_module,inspect_plugin_manifest} from '../plugin-pkg/validator.js';
const FRAME=16*1024*1024,OUTPUT=1024*1024,encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
let token,identity,exports,waiting=0,lastId=0,phase='new';
const ready=init();
function send(data){self.postMessage({token,identity,...data});}
function exact(value,keys){return value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).every(key=>keys.includes(key));}
function range(ptr,len){if(!Number.isInteger(ptr)||!Number.isInteger(len)||ptr<=0||len<=0||len>FRAME||ptr+len>exports.memory.buffer.byteLength)throw new Error('Invalid plugin memory range.');}
async function hash(bytes){return [...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(v=>v.toString(16).padStart(2,'0')).join('');}
function step(input){
    const bytes=encoder.encode(JSON.stringify(input));if(bytes.length>FRAME)throw new Error('Plugin input exceeds 16 MiB.');
    const p=exports.harness_alloc(bytes.length)>>>0;range(p,bytes.length);new Uint8Array(exports.memory.buffer,p,bytes.length).set(bytes);
    const packed=BigInt.asUintN(64,exports.harness_step(p,bytes.length)),q=Number(packed>>32n),n=Number(packed&0xffffffffn);range(q,n);
    if(p<q+n&&q<p+bytes.length)throw new Error('Plugin input and output allocations overlap.');
    const copy=new Uint8Array(exports.memory.buffer,q,n).slice();exports.harness_free(p,bytes.length);exports.harness_free(q,n);
    const out=JSON.parse(decoder.decode(copy));
    if(out.type==='done'&&exact(out,['type','output'])&&typeof out.output==='string'&&encoder.encode(out.output).length<=OUTPUT){phase='finished';send({type:'done',output:out.output});return;}
    if(out.type==='error'&&exact(out,['type','error'])&&typeof out.error==='string'){phase='finished';send({type:'error',error:out.error.slice(0,4096)});return;}
    if(out.type!=='host_call'||!exact(out,['type','request'])||!exact(out.request,['id','op','args'])||out.request.id!==lastId+1||out.request.id>128||typeof out.request.op!=='string'||!out.request.args||typeof out.request.args!=='object'||Array.isArray(out.request.args))throw new Error('Invalid plugin step output or operation limit exceeded.');
    lastId=waiting=out.request.id;phase='waiting';send({type:'host_call',request:out.request});
}
self.onmessage=async event=>{
    try{
        const data=event.data;
        if(phase==='new'){
            phase='initializing';token=data.token;
            if(typeof token!=='string')throw new Error('Invalid plugin request identity.');
            await ready;
            if(data.type==='inspect'){
                const verified=inspect_plugin_package(data.bytes);verified.descriptors=JSON.parse(inspect_plugin_manifest(JSON.stringify(verified.manifest)));
                phase='finished';send({type:'verified',verified});return;
            }
            if(data.type!=='run')throw new Error('Invalid worker operation.');
            const d=data.descriptor;identity=`${d.pluginId}\0${d.version}\0${d.digest}`;
            inspect_plugin_module(data.bytes);if(await hash(data.bytes)!==d.digest)throw new Error('Plugin runtime digest mismatch.');
            const module=await WebAssembly.compile(data.bytes);if(WebAssembly.Module.imports(module).length)throw new Error('Plugin imports are disabled.');
            exports=(await WebAssembly.instantiate(module,{})).exports;
            if(exports.harness_abi_version()!==1)throw new Error('Unsupported plugin ABI version.');
            phase='running';step({type:'start',request:data.request});return;
        }
        if(data.token!==token||phase!=='waiting'||data.type!=='resume'||data.response?.id!==waiting)throw new Error('Invalid plugin resume sequence.');
        phase='running';step({type:'resume',response:data.response});
    }catch(error){phase='finished';send({type:'error',error:String(error?.message??error).slice(0,4096)});}
};
