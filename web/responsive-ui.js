export function bindResponsiveDisclosure(details,{onLayout=()=>{},closeOnAction=false}={}){
    const mobile=matchMedia('(max-width:700px)'),summary=details.querySelector(':scope>summary');
    const layout=()=>{details.open=!mobile.matches;onLayout(mobile.matches);};
    mobile.addEventListener('change',layout);layout();
    function close(){if(mobile.matches&&details.open){details.open=false;summary.focus();}}
    document.addEventListener('keydown',event=>{if(event.key==='Escape'&&!event.defaultPrevented&&mobile.matches&&details.open&&!event.target.closest?.('dialog[open]')){event.preventDefault();close();}});
    if(closeOnAction)details.addEventListener('click',event=>{if(event.target.closest('button'))close();});
    document.addEventListener('pointerdown',event=>{if(mobile.matches&&!details.contains(event.target))details.open=false;});
}
