import {getProfileNames,loadProfile,saveProfile,setCurrentProfile,getCurrentProfile,migrateToProfiles} from './config.js';
export function loadConnections(storage){
    const warnings=[];try{migrateToProfiles(storage);}catch{warnings.push('Saved configuration needs attention; files and conversations remain available.');}
    const records=[];
    for(const id of getProfileNames(storage)){try{const settings=loadProfile(storage,id);if(settings)records.push({id,name:settings.connection_name || id,settings});}catch{warnings.push(`A saved connection is invalid and was not applied.`);}}
    return {records,current:getCurrentProfile(storage),warnings};
}
export function saveConnection(storage,id,settings){saveProfile(storage,id,settings);setCurrentProfile(storage,id);}
