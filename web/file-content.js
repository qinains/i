const escape=text=>text.replace(/[&<>"']/gu,char=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
export function markdownHtml(text){
    const inline=value=>escape(value).replace(/`([^`]+)`/gu,'<code>$1</code>').replace(/\*\*([^*]+)\*\*/gu,'<strong>$1</strong>');
    const output=[];let code=null;
    for(const line of text.split(/\r?\n/u)){
        if(/^\s*```/u.test(line)){if(code){output.push('<pre><code>'+escape(code.join('\n'))+'</code></pre>');code=null;}else code=[];continue;}
        if(code){code.push(line);continue;}
        const heading=/^(#{1,6})\s+(.+)$/u.exec(line);
        if(heading)output.push(`<h${heading[1].length}>${inline(heading[2])}</h${heading[1].length}>`);
        else if(/^\s*[-*+]\s+/u.test(line))output.push('<ul><li>'+inline(line.replace(/^\s*[-*+]\s+/u,''))+'</li></ul>');
        else if(line.trim())output.push('<p>'+inline(line)+'</p>');
    }
    if(code)output.push('<pre><code>'+escape(code.join('\n'))+'</code></pre>');return output.join('\n');
}
export function staticDocument(body){
    return '<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src \'none\'; img-src data:; style-src \'unsafe-inline\'; base-uri \'none\'; form-action \'none\'"><style>body{font:16px/1.6 system-ui,sans-serif;padding:20px;color:#202630;overflow-wrap:anywhere}pre{overflow:auto;white-space:pre-wrap;background:#f2f4f7;padding:12px}img{max-width:100%;height:auto}code{font-family:monospace}</style></head><body>'+body+'</body></html>';
}
export function textDocument(text){return staticDocument('<pre>'+escape(text)+'</pre>');}
