import {readFile} from 'node:fs/promises';
import {test,expect} from './fixtures.mjs';

test('large messages render a bounded preview and the download preserves all text',async({page})=>{
    const value='BEGIN '+ 'x'.repeat(128*1024)+' FULL MESSAGE TAIL';
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    const id=await page.locator('[role=tab][aria-selected=true]').getAttribute('data-session-id');
    await page.evaluate(async({id,value})=>{
        const {openStateStore}=await import('/state/store.js'),store=await openStateStore();
        try{
            const record=(await store.load()).sessions.find(session=>session.id===id);
            record.transcript.push({type:'user',text:value});
            await store.commitSession(record,record.revision);
        }finally{store.close();}
    },{id,value});
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');
    const line=page.locator('#output .line.user').last();
    await expect(line).toContainText('BEGIN');await expect(line).not.toContainText('FULL MESSAGE TAIL');
    expect((await line.textContent()).length).toBeLessThan(17*1024);
    const downloaded=page.waitForEvent('download');await line.getByRole('button',{name:'Download full message'}).click();
    const file=await downloaded;
    expect(file.suggestedFilename()).toBe('i-message.txt');
    expect(await readFile(await file.path(),'utf8')).toBe(value);
});
