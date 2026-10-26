import {test,expect} from './fixtures.mjs';
import {pluginPackage} from './plugin-fixtures.mjs';
test('plugin UI installs without enabling and restores workspace authorization after reload',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#plugins-btn').click();
    await expect(page.locator('#plugin-file')).toBeEnabled();await page.locator('#plugin-file').setInputFiles({name:'count.hplugin.zip',mimeType:'application/zip',buffer:Buffer.from(await pluginPackage())});
    await expect(page.locator('#plugin-preview')).toContainText('Unverified publisher');
    await expect(page.locator('#plugin-preview .plugin-security')).toHaveAttribute('open','');
    await expect(page.locator('#plugin-preview .plugin-security')).toContainText('No access to app credentials');
    await expect(page.locator('#plugin-preview .plugin-security')).toContainText('do not prove plugin code is harmless');
    await page.locator('#plugin-install').click();
    const card=page.locator('[data-plugin-id="example.word-count"]');await expect(card).toContainText('Disabled');
    page.once('dialog',d=>d.accept());await card.locator('[data-plugin-action=enable]').click();await expect(card).toContainText('Enabled');
    await page.reload();await expect(page.locator('#status')).toHaveText('Ready');await page.locator('#plugins-btn').click();await expect(card).toContainText('Enabled');
    page.once('dialog',d=>d.accept());await card.locator('[data-plugin-action=uninstall]').click();await expect(card).toHaveCount(0);
});
test('plugin UI Chinese controls fit phone and never render manifest names as HTML',async({page})=>{
    await page.setViewportSize({width:375,height:812});await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');await page.evaluate(()=>localStorage.setItem('i_lang','zh-CN'));await page.reload();await expect(page.locator('#status')).toHaveText('就绪');
    await expect(page.locator('#plugins-btn')).toHaveText('插件');await page.locator('#plugins-btn').click();
    await expect(page.locator('#plugin-file')).toBeEnabled();await page.locator('#plugin-file').setInputFiles({name:'count.hplugin.zip',mimeType:'application/zip',buffer:Buffer.from(await pluginPackage({name:'<img src=x onerror="window.pluginXSS=1">'}))});await expect(page.locator('#plugin-preview')).toContainText('未验证发布者');
    expect(await page.locator('#plugin-preview img').count()).toBe(0);expect(await page.evaluate(()=>window.pluginXSS)).toBeUndefined();
    expect(await page.evaluate(()=>document.querySelector('#plugins-modal').scrollWidth<=document.querySelector('#plugins-modal').clientWidth+1)).toBe(true);
});

test('Plugins is independent of Settings and keyboard close restores focus without stealing it',async({page})=>{
    await page.goto('/');await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('header #plugins-btn')).toBeVisible();await expect(page.locator('#settings-modal #plugins-btn,[data-settings-tab=plugins],[data-settings-panel=plugins]')).toHaveCount(0);
    await page.locator('#plugins-btn').click();await expect(page.locator('#plugins-modal')).toBeVisible();await expect(page.locator('#settings-modal')).not.toBeVisible();await expect(page.locator('#plugins-modal #save-config')).toHaveCount(0);
    await page.keyboard.press('Escape');await expect(page.locator('#plugins-modal')).not.toBeVisible();await expect(page.locator('#plugins-btn')).toBeFocused();
    await page.locator('#plugins-btn').click();await expect(page.locator('#plugin-file')).toBeEnabled();await page.evaluate(()=>{document.querySelector('#plugins-modal').close();document.querySelector('#input').focus();});await expect(page.locator('#input')).toBeFocused();
    await page.locator('#settings-btn').click();await expect(page.locator('[data-settings-tab]')).toHaveCount(4);await expect(page.locator('#save-config')).toBeVisible();
});

test('legacy Plugins deep link opens independent dialog and fits 320px LTR and RTL headers',async({page})=>{
    await page.setViewportSize({width:320,height:812});await page.goto('/?settings=plugins');await expect(page.locator('#status')).toHaveText('Ready');
    await expect(page.locator('#plugins-modal')).toBeVisible();await expect(page.locator('#settings-modal')).not.toBeVisible();await expect(page.locator('#plugin-file')).toBeEnabled();
    expect(await page.locator('#plugins-modal').evaluate(el=>el.scrollWidth<=el.clientWidth+1)).toBe(true);
    await page.locator('#close-plugins').click();
    for(const direction of ['ltr','rtl']){
        await page.evaluate(direction=>document.documentElement.dir=direction,direction);
        for(const id of ['plugins-btn','settings-btn']){const button=page.locator(`#${id}`);await expect(button).toBeVisible();expect(await button.evaluate(el=>{const box=el.getBoundingClientRect();return box.left>=0&&box.right<=innerWidth;})).toBe(true);}
        expect(await page.locator('header').evaluate(el=>el.scrollWidth<=el.clientWidth+1)).toBe(true);
    }
});
