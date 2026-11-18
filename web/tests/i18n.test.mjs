import test from 'node:test';
import assert from 'node:assert/strict';
import {t} from '../i18n.js';

for(const [language,labels]of Object.entries({
    en:['Send','Queue message','Stop'],
    'zh-CN':['发送','追加','停止'],
    'zh-TW':['傳送','追加','停止'],
    ja:['送信','追加','停止'],
    ko:['보내기','추가','중지'],
    fr:['Envoyer','Ajouter à la file','Arrêter'],
    de:['Senden','Einreihen','Stoppen'],
}))test(`composer actions are translated in ${language}`,()=>{
    assert.deepEqual(['Send','Queue message','Stop'].map(key=>t(key,language)),labels);
});
