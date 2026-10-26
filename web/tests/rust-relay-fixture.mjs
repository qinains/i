import http from 'node:http';
import {spawn,execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root=fileURLToPath(new URL('../../',import.meta.url)),relays=new Map(),children=new Set();
let executable;
function testExecutable(){
    if(executable)return executable;
    const output=execFileSync('cargo',['test','-p','i-relay','--lib','--no-run','--locked','--message-format=json'],{cwd:root,encoding:'utf8',stdio:['ignore','pipe','inherit']});
    for(const line of output.trim().split('\n')){
        const item=JSON.parse(line);
        if(item.reason==='compiler-artifact'&&item.target.name==='i_relay'&&item.profile.test&&item.executable)executable=item.executable;
    }
    if(!executable)throw new Error('Rust relay fixture executable was not built');
    return executable;
}
function start(origin){
    return new Promise((resolve,reject)=>{
        const env=Object.fromEntries(['PATH','HOME','RUSTUP_HOME','CARGO_HOME','TMPDIR','TMP','TEMP','SystemRoot'].filter(key=>process.env[key]!==undefined).map(key=>[key,process.env[key]]));
        const child=spawn(testExecutable(),['--exact','tests::browser_fixture','--ignored','--nocapture'],{cwd:root,env:{...env,HARNESS_TEST_ORIGIN:origin},stdio:['ignore','pipe','pipe']});
        children.add(child);child.stderr.resume();let output='';
        const timer=setTimeout(()=>{child.kill();reject(new Error('Rust relay fixture startup timed out'));},10000);
        child.on('error',()=>{clearTimeout(timer);reject(new Error('Rust relay fixture could not start'));});
        child.on('exit',()=>{children.delete(child);clearTimeout(timer);reject(new Error('Rust relay fixture stopped'));});
        child.stdout.on('data',bytes=>{
            output=(output+bytes).slice(-8192);
            const match=output.match(/HARNESS_RELAY_FIXTURE (\{"port":\d+\})/u);
            if(!match)return;const {port}=JSON.parse(match[1]);
            if(!Number.isInteger(port)||port<1||port>65535)return;
            clearTimeout(timer);resolve(`http://127.0.0.1:${port}/api/chat/completions`);
        });
    });
}
process.once('exit',()=>{for(const child of children)child.kill();});

export async function forwardRustRelay(req,res){
    const origin=`http://${req.headers.host}`;
    if(!/^http:\/\/127\.0\.0\.1:\d+$/u.test(origin)){res.writeHead(403);res.end();return;}
    try{
        if(!relays.has(origin))relays.set(origin,start(origin));
        const url=await relays.get(origin);
        const proxy=http.request(url,{method:req.method,headers:req.headers},upstream=>{
            res.writeHead(upstream.statusCode,upstream.headers);upstream.pipe(res);
            upstream.on('error',()=>res.destroy());res.on('close',()=>upstream.destroy());
        });
        proxy.on('error',()=>{if(res.destroyed||res.writableEnded)return;if(res.headersSent){res.destroy();return;}res.writeHead(502);res.end('Rust relay fixture failed');});
        req.on('aborted',()=>proxy.destroy());res.on('close',()=>proxy.destroy());req.pipe(proxy);
    }catch{res.writeHead(502);res.end('Rust relay fixture failed');}
}
