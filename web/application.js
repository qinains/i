import {Context} from './plugins/vendor/cordis.js';
import {SessionController} from './sessions.js';
import {createApplicationPlugins} from './plugins/application.js';

export function createBrowserApplication({Agent,store,hooks,openPluginStore,inspectPackage}){
    const context=new Context(),controller=new SessionController(Agent,store,hooks,context);
    let terminal=false,pluginApplication,shutdownPromise;
    const errors=[];
    const beginClose=()=>{terminal=true;controller.closing=true;controller.cancelAll();};
    context.effect(()=>async()=>{
        beginClose();
        await controller.shutdown();
        const sessions=await Promise.all([...controller.closes.values()]);
        errors.push(...sessions.flatMap(report=>report.errors));
        if(pluginApplication){
            try{const plugins=await pluginApplication;const report=await plugins.shutdown();errors.push(...report.errors);}
            catch(error){errors.push(String(error?.message??error));}
        }
        if(sessions.every(report=>report.closed)){
            try{await store.close();}catch(error){errors.push(String(error?.message??error));}
        }
    });
    return {
        context,controller,
        async plugins(){
            if(terminal)throw new Error('Application is closing.');
            pluginApplication??=createApplicationPlugins({context,store:openPluginStore,inspectPackage});
            try{
                const plugins=await pluginApplication;
                if(terminal)throw new Error('Application is closing.');
                return plugins;
            }catch(error){if(!terminal)pluginApplication=null;throw error;}
        },
        shutdown({timeoutMs=10000}={}){
            if(shutdownPromise)return shutdownPromise;
            if(!Number.isSafeInteger(timeoutMs)||timeoutMs<1)throw new Error('Invalid shutdown timeout.');
            beginClose();
            const drained=context.fiber.dispose().then(()=>({
                closed:[...controller.closeResults.values()].every(report=>report.closed),
                errors:[...errors],retained:[...controller.closeResults].filter(([,report])=>!report.closed).map(([id])=>id),
            }));
            shutdownPromise=new Promise(resolve=>{
                const timer=setTimeout(()=>resolve({closed:false,errors:['Application shutdown timed out; in-use resources were retained.'],retained:[...controller.scopes.keys()].filter(id=>!controller.closeResults.get(id)?.closed)}),timeoutMs);
                drained.then(report=>{clearTimeout(timer);resolve(report);},error=>{clearTimeout(timer);resolve({closed:false,errors:[String(error?.message??error)],retained:[...controller.scopes.keys()]});});
            });
            return shutdownPromise;
        },
        requestShutdown(options){void this.shutdown(options);},
    };
}
