import test from 'node:test';
import assert from 'node:assert/strict';
import {installPreviewStorage} from '../preview-storage.js';
test('preview storage supports methods and named keys in its own realm only',()=>{
    const host={};Object.defineProperty(host,'localStorage',{get(){throw new Error('Opaque storage');},configurable:true});
    installPreviewStorage(host);const local=host.localStorage;
    assert.equal(local.getItem('absent'),null);local.setItem('count',1);local.theme='dark';local.setItem('__proto__','safe');
    assert.equal(local.getItem('count'),'1');assert.equal(local.theme,'dark');assert.equal(local.length,3);assert.equal(local.key(0),'count');assert.equal(local.key(99),null);assert.equal(Object.getPrototypeOf(local),null);
    assert.equal(Object.prototype.toString.call(local),'[object Storage]');assert.deepEqual(Object.keys(local),['count','theme','__proto__']);
    delete local.theme;assert.equal(local.getItem('theme'),null);local.removeItem('count');assert.equal(local.length,1);local.clear();assert.equal(local.length,0);
    host.sessionStorage.setItem('draft','x');assert.equal(local.getItem('draft'),null);
    const other={};installPreviewStorage(other);assert.equal(other.sessionStorage.getItem('draft'),null);
});
test('preview local and session storage share a bounded quota and failed writes are atomic',()=>{
    const host={};installPreviewStorage(host,{maxBytes:16});const {localStorage:local,sessionStorage:session}=host;
    local.setItem('a','123');session.setItem('b','123');
    assert.throws(()=>local.setItem('a','longer'),error=>error.name==='QuotaExceededError');assert.equal(local.getItem('a'),'123');
    session.clear();local.setItem('a','longer');assert.equal(local.getItem('a'),'longer');
    local.removeItem('a');session.setItem('b','123');assert.equal(session.length,1);
});
