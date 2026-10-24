export function modelDeadline(parent,milliseconds=30*60*1000,timers=globalThis){
    const controller=new AbortController(),signal=AbortSignal.any([parent,controller.signal]);
    let timer,disposed=false;
    const clear=()=>{timers.clearTimeout(timer);timer=undefined;};
    function touch(){
        clear();if(disposed||signal.aborted)return;
        timer=timers.setTimeout(()=>controller.abort(new DOMException('No model response data received for 30 minutes. Previously committed files are retained; retry manually or check the endpoint.','TimeoutError')),milliseconds);
        timer?.unref?.();
    }
    signal.addEventListener('abort',clear,{once:true});touch();
    return {signal,touch,dispose(){disposed=true;clear();signal.removeEventListener('abort',clear);}};
}
