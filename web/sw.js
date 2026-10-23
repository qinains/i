/* Build script prefixes the verified static asset list into service-worker.js. */
const {version,precache} = self.HARNESS_BUILD;
const scope = self.registration.scope;
const prefix = `harness-pwa:${scope}:`;
const cacheName = prefix+version;
const urls = new Set(precache.map(asset => new URL(asset.url,scope).href));
self.addEventListener('install',event => event.waitUntil((async () => {
    const cache = await caches.open(cacheName);
    try {
        for (const asset of precache) {
            const url = new URL(asset.url,scope).href;
            const response = await fetch(url,{cache:'reload',credentials:'omit',redirect:'error'});
            if (!response.ok) throw new Error('Missing PWA asset');
            const bytes = await response.clone().arrayBuffer();
            const digest = [...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(b => b.toString(16).padStart(2,'0')).join('');
            if (digest !== asset.sha256) throw new Error('PWA asset version mismatch');
            await cache.put(url,response);
        }
    } catch (error) {await caches.delete(cacheName); throw error;}
})()));
self.addEventListener('activate',event => event.waitUntil((async () => {
    for (const name of await caches.keys()) if (name.startsWith(prefix) && name !== cacheName) await caches.delete(name);
    await self.clients.claim();
})()));
self.addEventListener('fetch',event => {
    if (event.request.method !== 'GET') return;
    const url = new URL(event.request.url);
    if (url.search || url.origin !== new URL(scope).origin) return;
    const target = url.href === scope ? new URL('index.html',scope).href : url.href;
    if (!urls.has(target)) return;
    event.respondWith((async () => (await caches.open(cacheName)).match(target).then(cached => cached || fetch(event.request)))());
});
let updating;
const replies = new Map();
async function activateSafely() {
    if (updating) return;
    updating = true;
    const id = crypto.randomUUID();
    let pages = [];
    try {
        pages = (await self.clients.matchAll({type:'window',includeUncontrolled:true})).filter(client => client.url.startsWith(scope));
        if (!pages.length) return;
        const acks = pages.map(page => new Promise(resolve => {
            const timer = setTimeout(() => {replies.delete(page.id); resolve(false);},3000);
            replies.set(page.id,{id,resolve:value => {clearTimeout(timer); replies.delete(page.id); resolve(value);}});
            page.postMessage({type:'pwa_lock',id});
        }));
        const ready = (await Promise.all(acks)).every(Boolean);
        const current = (await self.clients.matchAll({type:'window',includeUncontrolled:true})).filter(client => client.url.startsWith(scope));
        if (!ready || current.some(client => !pages.some(page => page.id === client.id))) throw new Error('Another tab is busy or unavailable');
        await self.skipWaiting();
    } catch {
        for (const page of pages) page.postMessage({type:'pwa_unlock',id});
    } finally {updating = false;}
}
self.addEventListener('message',event => {
    if (!event.source?.url?.startsWith(scope)) return;
    if (event.data?.type === 'pwa_update') event.waitUntil(activateSafely());
    if (event.data?.type === 'pwa_idle') {
        const reply = replies.get(event.source.id);
        if (reply?.id === event.data.id) reply.resolve(event.data.idle === true);
    }
});
