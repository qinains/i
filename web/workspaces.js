const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
export function isTemporaryWorkspace(id){return typeof id==='string'&&id.startsWith('temp-')&&UUID.test(id.slice(5));}
export function validateWorkspaceId(id){if(id!=='default' && !UUID.test(id) && !isTemporaryWorkspace(id))throw new Error('Invalid workspace ID.');return id;}
export function browserPath(value){
    if(typeof value!=='string'||/[\\\0:]/u.test(value)||value.split('/').includes('..'))throw new Error('Invalid browser file path.');
    let mount='workspace',path=value;
    if(value==='/tmp'||value.startsWith('/tmp/')){mount='tmp';path=value.slice(4);}
    else if(value==='/workspace'||value.startsWith('/workspace/'))path=value.slice(10);
    else if(value.startsWith('/'))throw new Error('Only /workspace and /tmp mounts are supported.');
    return {mount,path:path.split('/').filter(part=>part&&part!=='.').join('/')};
}
export function displayName(value){const name=String(value).trim();if(!name || name.length>80)throw new Error('Name must contain 1–80 characters.');return name;}
export async function listWorkspaceDirectories(){
    const root=await navigator.storage.getDirectory();const result=['default'];
    try{const dir=await (await root.getDirectoryHandle('harness')).getDirectoryHandle('workspaces');for await(const [name,handle]of dir.entries())if(handle.kind==='directory'&&UUID.test(name))result.push(name);}
    catch(error){if(error.name!=='NotFoundError')throw error;}
    return result;
}
