export async function registerPwa({isBusy,onLock,onStatus}) {
    if (!('serviceWorker' in navigator) || !isSecureContext) {onStatus('PWA requires HTTPS and Service Worker support.'); return;}
    const root = new URL('./',import.meta.url);
    const workerUrl = new URL('service-worker.js',root).href;
    const button = document.getElementById('btn-install');
    const update = document.getElementById('btn-update');
    let prompt, locked = false, lockId, lockTimer, requestTimer;
    let registration;
    window.addEventListener('beforeinstallprompt',event => {event.preventDefault(); prompt = event; button.hidden = false;});
    button.addEventListener('click',async () => {
        if (prompt) {await prompt.prompt(); prompt = null; button.hidden = true;}
    });
    navigator.serviceWorker.addEventListener('message',event => {
        if (event.data?.type === 'pwa_lock') {
            if (event.source?.scriptURL !== workerUrl) {
                event.source?.postMessage({type:'pwa_idle',id:event.data.id,idle:true}); return;
            }
            clearTimeout(requestTimer);
            const idle = !isBusy();
            if (idle) {
                locked = true; lockId = event.data.id; onLock(true);
                clearTimeout(lockTimer);
                lockTimer = setTimeout(() => {
                    if (!locked || registration?.waiting?.state === 'activating') return;
                    locked = false; onLock(false); onStatus('Update postponed; retry when all tabs are ready.');
                },10000);
            }
            event.source?.postMessage({type:'pwa_idle',id:event.data.id,idle});
        }
        if (event.data?.type === 'pwa_unlock' && lockId === event.data.id) {
            clearTimeout(lockTimer); locked = false; onLock(false); onStatus('Update postponed: another tab is busy or unavailable.');
        }
    });
    navigator.serviceWorker.addEventListener('controllerchange',() => {if (locked) location.reload();});
    registration = await navigator.serviceWorker.register(workerUrl,{scope:root.pathname,updateViaCache:'none'});
    function waiting() {
        if (registration.waiting) {update.hidden = false; onStatus('Update available. Finish tasks in all tabs, then apply.');}
    }
    registration.addEventListener('updatefound',() => {
        registration.installing?.addEventListener('statechange',() => {
            waiting();
            if (registration.active && !registration.waiting) onStatus(navigator.onLine ? 'PWA ready; local files are available offline.' : 'Offline: local files only.');
        });
    });
    update.addEventListener('click',() => {
        if (isBusy()) {onStatus('Finish the active task before updating.'); return;}
        clearTimeout(requestTimer);
        onStatus('Checking open app tabs before updating...');
        requestTimer = setTimeout(() => {
            if (!locked) onStatus('Update postponed: worker did not respond. Close all app tabs/windows and reopen to apply.');
        },4000);
        registration.waiting?.postMessage({type:'pwa_update'});
    });
    const online = () => onStatus(navigator.onLine ? 'Online. AI requires a reachable endpoint.' : 'Offline: AI unavailable; local files and ZIP export remain available.');
    window.addEventListener('online',online); window.addEventListener('offline',online); online(); waiting();
}
