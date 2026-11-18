import test from 'node:test';
import assert from 'node:assert/strict';
import {createTheme} from '../theme.js';
function setup(saved=null,dark=false){
    const data=new Map(saved?[['i_harness_theme',saved]]:[]),root={dataset:{},style:{}},media={matches:dark,addEventListener(_,fn){this.change=fn;}},meta={content:''};
    const storage={getItem:key=>data.get(key)??null,setItem:(key,value)=>data.set(key,value)};
    return {root,media,meta,data,theme:createTheme({root,media,meta,storage})};
}
test('automatic theme follows system changes while explicit modes stay selected',()=>{
    const {root,media,theme,data,meta}=setup();
    assert.equal(root.dataset.theme,'light');assert.equal(theme.mode,'auto');
    media.matches=true;media.change();assert.equal(root.dataset.theme,'dark');
    theme.setMode('light');assert.equal(root.dataset.theme,'light');assert.equal(data.get('i_harness_theme'),'light');
    media.change();assert.equal(root.dataset.theme,'light');assert.equal(meta.content,'#f3f5f8');
    theme.setMode('auto');assert.equal(root.dataset.theme,'dark');
    assert.throws(()=>theme.setMode('invalid'));assert.equal(theme.mode,'auto');
});
test('saved theme restores and invalid values fall back without configuration access',()=>{
    assert.equal(setup('dark').root.dataset.theme,'dark');
    assert.equal(setup('invalid').theme.mode,'auto');
    const keys=[];const root={dataset:{},style:{}};
    const theme=createTheme({root,media:{matches:false,addEventListener(){}},storage:{getItem(key){keys.push(key);throw new Error('blocked');},setItem(){throw new Error('blocked');}}});
    assert.equal(theme.setMode('dark'),false);assert.equal(root.dataset.theme,'dark');assert.deepEqual(keys,['i_harness_theme']);
});
