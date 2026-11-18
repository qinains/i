import {readFile} from 'node:fs/promises';
import {gzipSync,brotliCompressSync,constants} from 'node:zlib';
const files=process.argv.slice(2);
if(!files.length)files.push('web/pkg/i_bg.wasm','web/plugin-pkg/validator_bg.wasm');
for(const path of files){
    const bytes=await readFile(path);
    console.log(JSON.stringify({path,raw:bytes.length,gzip:gzipSync(bytes,{level:9}).length,
        brotli:brotliCompressSync(bytes,{params:{[constants.BROTLI_PARAM_QUALITY]:11}}).length}));
}
