// Run inside the browser so transparent controls are measured against their real surface.
export function controlMetrics(element){
    if(Array.isArray(element))return element.flatMap(node=>{
        const normal=controlMetrics(node);if(normal.disabled)return [normal];
        node.focus({preventScroll:true});const focused=controlMetrics(node);focused.label+=' (focus)';
        return [normal,focused];
    });
    const rgba=value=>(value.match(/[\d.]+/g)||[]).map(Number);
    const blend=(front,back)=>{const alpha=front[3]??1;return front.slice(0,3).map((value,index)=>value*alpha+back[index]*(1-alpha));};
    const surface=node=>{
        if(!node)return [255,255,255];
        const style=getComputedStyle(node),color=rgba(style.backgroundColor);
        return blend(color,surface(node.parentElement));
    };
    const style=getComputedStyle(element),background=surface(element),text=blend(rgba(style.color),background);
    const luminance=rgb=>rgb.map(value=>{const v=value/255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4;}).reduce((sum,v,index)=>sum+v*[.2126,.7152,.0722][index],0);
    const values=[luminance(text),luminance(background)].sort((a,b)=>b-a);
    const rect=element.getBoundingClientRect(),iconOnly=!element.textContent.trim()&&!!element.querySelector('svg');
    return {label:element.id||element.getAttribute('aria-label')||element.textContent.trim().slice(0,60),ratio:(values[0]+.05)/(values[1]+.05),iconOnly,disabled:element.matches(':disabled,[aria-disabled=true]'),opacity:Number(style.opacity),width:rect.width,height:rect.height};
}
