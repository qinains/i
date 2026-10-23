import {test, expect} from './fixtures.mjs';
test('OPFS persists nested Unicode and binary data across reload', async ({page}) => {
    await page.goto('/');
    await page.evaluate(async () => {
        const fs = await import('/platform.js'); const root = await fs.opfsOpen();
        await fs.opfsWrite(root, 'nested/中文.bin', new Uint8Array([0,255,10]));
        await fs.opfsMkdir(root, 'empty');
    });
    await page.reload();
    const data = await page.evaluate(async () => {
        const fs = await import('/platform.js'); const root = await fs.opfsOpen();
        return {bytes: [...await fs.opfsRead(root,'nested/中文.bin',10)], files: await fs.opfsList(root,''), missing: await fs.opfsMetadata(root,'absent')};
    });
    expect(data).toEqual({bytes:[0,255,10], files:['empty','nested'], missing:null});
});
