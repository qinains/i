import {displayName} from '../workspaces.js';
export function legacyWorkspace(storage){
    const raw=storage.getItem('i_workspaces');if(!raw)return null;
    const value=JSON.parse(raw);
    if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('Invalid legacy workspace labels.');
    const names=Object.keys(value);if(!names.length)return null;
    const selected=storage.getItem('i_current_workspace');
    return {source:'i_workspaces',version:1,name:displayName(names.includes(selected)?selected:names[0]),labels:names};
}
