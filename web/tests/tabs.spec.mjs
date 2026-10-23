import {test,expect} from './fixtures.mjs';
test('tabs contain compact close icons and a minimal logo, including touch layout',async({page},testInfo)=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('.brand')).toHaveAttribute('aria-label','lninl');await expect(page.locator('.brand')).toHaveText('lninl');await expect(page.locator('.brand-i')).toHaveText('i');
    const brand=await page.locator('.brand').evaluate(el=>{const base=getComputedStyle(el),mark=getComputedStyle(el.querySelector('.brand-i'));return {base:base.color,mark:mark.color,baseWeight:base.fontWeight,markWeight:mark.fontWeight};});
    expect(brand.mark).not.toBe(brand.base);expect(Number(brand.markWeight)).toBeGreaterThan(Number(brand.baseWeight));
    expect(await page.locator('.brand').evaluate(el=>getComputedStyle(el).backgroundColor)).toBe('rgba(0, 0, 0, 0)');
    const first=await page.locator('[role=tab]').getAttribute('data-session-id');
    await page.locator('#btn-new-session').click();await expect(page.locator('.session-tab')).toHaveCount(2);
    const group=page.locator('.session-tab').filter({has:page.locator(`[data-session-id="${first}"]`)});
    await expect(group.locator('.tab-close svg')).toHaveCount(1);
    await group.hover();await page.screenshot({path:testInfo.outputPath('header-tabs.png')});
    await group.locator('.tab-close').click();await expect(page.locator('[role=tab]')).toHaveCount(1);
    await page.setViewportSize({width:390,height:844});
    const button=page.locator('.tab-close'),box=await button.boundingBox();expect(box.width).toBeGreaterThanOrEqual(40);expect(box.height).toBeGreaterThanOrEqual(40);
    expect(await button.evaluate(el=>getComputedStyle(el).opacity)).toBe('1');
    await page.screenshot({path:testInfo.outputPath('header-tabs-mobile.png')});
});
test('closed tabs reopen from history with safe names and preserved drafts',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    const first=page.locator('[role=tab]').first();const id=await first.getAttribute('data-session-id');
    page.once('dialog',dialog=>dialog.accept('<img src=x onerror="window.injected=true">'));await first.dblclick();
    await page.locator('#input').fill('draft one');await expect(page.locator('#save-status')).toHaveText('Saved');await page.locator('#btn-new-session').click();await expect(page.locator('[role=tab][aria-selected=true]')).not.toHaveAttribute('data-session-id',id);
    await page.locator('#input').fill('draft two');await page.locator(`[data-session-id="${id}"]`).click();await expect(page.locator('#input')).toHaveValue('draft one');
    await page.locator(`[data-session-id="${id}"] + button`).click();await expect(page.locator('#input')).toHaveValue('draft two');
    await page.locator('#btn-history').click();await page.locator('#history-list button').filter({hasText:'<img'}).click();await expect(page.locator('#input')).toHaveValue('draft one');
    expect(await page.evaluate(()=>window.injected===true)).toBe(false);await expect(page.locator('#session-tabs img')).toHaveCount(0);
    await expect.poll(()=>page.evaluate(async()=>{const {openStateStore}=await import('/state/store.js');const store=await openStateStore();const state=await store.load();store.close();return state.ui?.activeSessionId;})).toBe(id);
    await page.reload();await expect(page.locator('[role=tab][aria-selected=true]')).toHaveAttribute('data-session-id',id);await expect(page.locator('#input')).toHaveValue('draft one');
});
test('background approval stays with its owning session',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');const first=await page.locator('[role=tab]').getAttribute('data-session-id');await page.locator('#btn-new-session').click();await expect(page.locator('[role=tab][aria-selected=true]')).not.toHaveAttribute('data-session-id',first);
    const second=await page.locator('[role=tab][aria-selected=true]').getAttribute('data-session-id');await page.locator('#settings-btn').click();await page.locator('#base-url').fill('http://127.0.0.1:4173/mock/v1');await page.locator('#model').fill('fixture');await page.locator('#save-config').click();await page.locator('#close-settings').click();
    await page.locator('#input').fill('MAKE');await page.locator('#input').press('Enter');await expect(page.locator('#approval-panel')).toBeVisible();
    const close=page.locator(`[data-session-id="${second}"] + button`);await expect(close).toBeDisabled();expect(Number(await close.evaluate(el=>getComputedStyle(el).opacity))).toBeLessThan(.6);
    await page.locator(`[data-session-id="${first}"]`).click();await expect(page.locator('#approval-panel')).toBeHidden();await expect(page.locator('#running-notice')).toBeVisible();await expect(page.locator('#output')).not.toContainText('> MAKE');
    await page.locator('#jump-running').click();await expect(page.locator('[role=tab][aria-selected=true]')).toHaveAttribute('data-session-id',second);await expect(page.locator('#approval-panel')).toBeVisible();await page.locator('#btn-cancel').click();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#file-list')).toContainText('No files');
});
test('legacy shared labels migrate once without moving or splitting old files',async({page})=>{
    await page.addInitScript(()=>{if(!localStorage.getItem('i_workspaces')){localStorage.setItem('i_workspaces',JSON.stringify({A:{},B:{}}));localStorage.setItem('i_current_workspace','B');}});
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#workspace-select option:checked')).toHaveText('B');await expect(page.locator('#workspace-select option:not([disabled]):not([value="__new__"]):not([value="__scratch__"])')).toHaveCount(1);await expect(page.locator('#global-error')).toContainText('shared one file directory');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await expect(page.locator('#workspace-select option:checked')).toHaveText('B');
});
