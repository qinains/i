export function validateSession(record){
    if(!record || typeof record.id!=='string' || typeof record.workspaceId!=='string' || typeof record.name!=='string'
        || typeof record.connectionId!=='string' || typeof record.connectionIdentity!=='string' || typeof record.draft!=='string'
        || !Number.isSafeInteger(record.revision)||record.revision<0 || !Array.isArray(record.history)||!Array.isArray(record.transcript)
        || record.transcript.some(line=>!line || !['system','assistant','reasoning','user','error','tool','tool_result','success'].includes(line.type)||typeof line.text!=='string'||(line.pending!=null&&typeof line.pending!=='boolean')||(line.expanded!=null&&typeof line.expanded!=='boolean'))
        || (record.mode!=null&&!['ask','auto'].includes(record.mode))
        || (record.titleSource!=null&&!['default','first','compact','manual'].includes(record.titleSource))||(record.titleText!=null&&typeof record.titleText!=='string')
        || !['idle','running','approval','interrupted'].includes(record.status))throw new Error('Invalid saved session.');
    validatePending(record);return record;
}
import {validatePending} from '../pending.js';
