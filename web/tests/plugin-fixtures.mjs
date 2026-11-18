import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
export async function pluginPackage(overrides={},wasmName='word-count'){
    const wasm=await readFile(new URL(`../../target/browser-plugin-fixtures/${wasmName}.wasm`,import.meta.url));
    const base=JSON.parse(await readFile(new URL('../../examples/wasm-word-count/manifest.json',import.meta.url),'utf8'));
    const manifest={...base,...overrides,sha256:createHash('sha256').update(wasm).digest('hex')};
    const locals=[],centrals=[];let offset=0;
    for(const [name,data]of [['manifest.json',Buffer.from(JSON.stringify(manifest))],['plugin.wasm',wasm],['LICENSE',Buffer.from('MIT')]]){
        const file=Buffer.from(name),crc=checksum(data),local=Buffer.alloc(30),central=Buffer.alloc(46);
        local.writeUInt32LE(0x04034b50);local.writeUInt16LE(20,4);local.writeUInt32LE(crc,14);local.writeUInt32LE(data.length,18);local.writeUInt32LE(data.length,22);local.writeUInt16LE(file.length,26);
        central.writeUInt32LE(0x02014b50);central.writeUInt16LE(20,4);central.writeUInt16LE(20,6);central.writeUInt32LE(crc,16);central.writeUInt32LE(data.length,20);central.writeUInt32LE(data.length,24);central.writeUInt16LE(file.length,28);central.writeUInt32LE(offset,42);
        locals.push(local,file,data);centrals.push(central,file);offset+=local.length+file.length+data.length;
    }
    const directory=Buffer.concat(centrals),end=Buffer.alloc(22);end.writeUInt32LE(0x06054b50);end.writeUInt16LE(3,8);end.writeUInt16LE(3,10);end.writeUInt32LE(directory.length,12);end.writeUInt32LE(offset,16);
    return [...Buffer.concat([...locals,directory,end])];
}
function checksum(data){let crc=0xffffffff;for(const byte of data){crc^=byte;for(let i=0;i<8;i++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}return (crc^0xffffffff)>>>0;}
