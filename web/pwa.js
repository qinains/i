export async function registerPwa({isBusy,onLock,onStatus,beforeUpdate=async()=>{}}) {
    if (!navigator.serviceWorker || !isSecureContext) {onStatus('PWA requires HTTPS and Service Worker support.'); return;}
    const root = new URL('./',import.meta.url);
    const workerUrl = new URL('service-worker.js',root).href;
    const button = document.getElementById('btn-install');
    const update = document.getElementById('btn-update');
    let prompt, locked = false, attempt, lockTimer, requestTimer, requested = false;
    let registration;
    function release() {
        clearTimeout(lockTimer); clearTimeout(requestTimer);
        attempt = undefined; locked = false; requested = false;
        update.disabled = false; onLock(false);
    }
    const postponed = {
        busy:'Update postponed: another app tab is running a task. Finish or stop it, then retry. Your files and settings have not been cleared.',
        'save-failed':'Update postponed: a page could not save its state. Check its save status and back up your work before retrying. Nothing has been cleared.',
        changed:'Update postponed: another app page opened during the check. Retry when all app pages are ready.',
        unresponsive:'Update postponed: an app page did not respond. Bring other app tabs or PWA windows to the foreground, or close idle ones, then retry. Do not clear site data.',
    };
    window.addEventListener('beforeinstallprompt',event => {event.preventDefault(); prompt = event; button.hidden = false;});
    button.addEventListener('click',async () => {
        if (prompt) {await prompt.prompt(); prompt = null; button.hidden = true;}
    });
    navigator.serviceWorker.addEventListener('message',async event => {
        if (event.data?.type === 'pwa_lock') {
            if (event.source?.scriptURL !== workerUrl) {
                event.source?.postMessage({type:'pwa_idle',id:event.data.id,idle:true,protocol:2}); return;
            }
            if (attempt?.id === event.data.id) return;
            clearTimeout(requestTimer); clearTimeout(lockTimer);
            const current = {id:event.data.id}; attempt = current;
            lockTimer = setTimeout(() => {
                if (attempt !== current || registration?.waiting?.state === 'activating') return;
                release(); onStatus(postponed.unresponsive);
            },60000);
            if (isBusy()) {
                event.source.postMessage({type:'pwa_idle',id:current.id,idle:false,reason:'busy'});
                return;
            }
            locked = true; update.disabled = true; onLock(true);
            onStatus('Saving state and checking other app pages… This may take up to 30 seconds.');
            try {
                await beforeUpdate();
                // A timed-out save may finish after its update attempt was cancelled.
                if (attempt !== current) return;
                event.source.postMessage({type:'pwa_idle',id:current.id,idle:true,protocol:2});
            } catch {
                if (attempt !== current) return;
                event.source.postMessage({type:'pwa_idle',id:current.id,idle:false,reason:'save-failed'});
                release(); onStatus(postponed['save-failed']);
            }
        }
        if (event.data?.type === 'pwa_unlock' && event.source?.scriptURL === workerUrl && attempt?.id === event.data.id) {
            release(); onStatus(postponed[event.data.reason] ?? postponed.unresponsive);
        }
    });
    navigator.serviceWorker.addEventListener('controllerchange',() => {if (locked) location.reload();else waiting();});
    registration = await navigator.serviceWorker.register(workerUrl,{scope:root.pathname,updateViaCache:'none'});
    if(!registration){onStatus('PWA registration unavailable; local workspace remains usable.');return;}
    function waiting() {
        if (!registration) return;
        update.hidden = !registration.active || !registration.waiting;
        if (!update.hidden && !attempt && !requested) onStatus('Update available. Finish tasks in all app tabs and download temporary files before applying. Saved workspace data is retained.');
    }
    registration.addEventListener('updatefound',() => {
        registration.installing?.addEventListener('statechange',() => {
            waiting();
            if (registration.active && !registration.waiting) onStatus(navigator.onLine ? 'PWA ready; local files are available offline.' : 'Offline: local files only.');
        });
    });
    update.addEventListener('click',async () => {
        if (requested || attempt) return;
        if (isBusy()) {onStatus('Finish or stop the current task before updating.'); return;}
        if (!registration.waiting) {onStatus('No update is waiting. Reopen the app to check again.');return;}
        requested = true; update.disabled = true;
        onStatus('Checking open app tabs before updating… This may take up to 30 seconds.');
        requestTimer = setTimeout(() => {
            release(); onStatus('Update postponed: the update service did not respond. Close idle app tabs and PWA windows, then reopen. Do not clear site data.');
        },35000);
        registration.waiting.postMessage({type:'pwa_update'});
    });
    const online = () => {if (!requested && !attempt) onStatus(navigator.onLine ? 'Online. AI requires a reachable endpoint.' : 'Offline: AI unavailable; local files and ZIP export remain available.');};
    window.addEventListener('online',online); window.addEventListener('offline',online); online(); waiting();
}
