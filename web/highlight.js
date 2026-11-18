// Lightweight syntax highlighter for preview source view.
// Supports: HTML, CSS, JS/MJS, JSON, TOML, and auto-detection.
const escapeMap={'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'};
function escape(text){return text.replace(/[&<>"']/g,ch=>escapeMap[ch]);}

const jsKeywords='\\b(?:abstract|arguments|async|await|boolean|break|byte|case|catch|char|class|const|continue|debugger|default|delete|do|double|else|enum|export|extends|false|final|finally|float|for|from|function|get|goto|if|implements|import|in|instanceof|int|interface|let|long|native|new|null|of|package|private|protected|public|return|set|short|static|super|switch|synchronized|this|throw|throws|transient|true|try|typeof|undefined|var|void|volatile|while|with|yield)\\b';
const cssKeywords='\\b(?:important|inherit|initial|unset|none|auto|block|inline|flex|grid|absolute|relative|fixed|sticky|hidden|visible|scroll|solid|dashed|dotted|bold|italic|center|left|right|top|bottom|normal|wrap|nowrap|pointer|default|transparent|currentColor)\\b';

const rules={
    html:[
        ['com',/<!--[\s\S]*?-->/],
        ['tag',/<\/?[\w-]+/],
        ['attr',/\b[\w-]+(?==)/],
        ['str',/"[^"]*"/],
        ['str',/'[^']*'/],
        ['brk',/[<>\/]/],
    ],
    css:[
        ['com',/\/\*[\s\S]*?\*\//],
        ['str',/"(?:[^"\\]|\\.)*"/],
        ['str',/'(?:[^'\\]|\\.)*'/],
        ['prop',/[\w-]+(?=\s*:)/],
        ['num',/\b\d+(?:\.\d+)?(?:px|em|rem|%|vh|vw|s|ms|deg|fr)?\b/],
        ['kw',new RegExp(cssKeywords)],
        ['val',/#[0-9a-fA-F]{3,8}\b/],
    ],
    js:[
        ['com',/\/\/[^\n]*/],
        ['com',/\/\*[\s\S]*?\*\//],
        ['str',/`(?:[^`\\]|\\.)*`/],
        ['str',/"(?:[^"\\]|\\.)*"/],
        ['str',/'(?:[^'\\]|\\.)*'/],
        ['num',/\b(?:0[xX][0-9a-fA-F]+|0[oO][0-7]+|0[bB][01]+|\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)\b/],
        ['kw',new RegExp(jsKeywords)],
    ],
    json:[
        ['key',/"(?:[^"\\]|\\.)*"(?=\s*:)/],
        ['str',/"(?:[^"\\]|\\.)*"/],
        ['num',/\b-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/],
        ['kw',/\b(?:true|false|null)\b/],
    ],
    toml:[
        ['com',/#[^\n]*/],
        ['sec',/\[[\w.". -]+\]/],
        ['key',/[\w.-]+(?=\s*=)/],
        ['str',/"(?:[^"\\]|\\.)*"/],
        ['str',/'[^']*'/],
        ['num',/\b\d+(?:\.\d+)?\b/],
        ['kw',/\b(?:true|false)\b/],
    ],
};

const colors={com:'#7a8594',str:'#a3d9a5',num:'#d9c97c',kw:'#7ab4f5',tag:'#e0937e',attr:'#c9a6e0',brk:'#8e9ba9',key:'#7ab4f5',val:'#d9c97c',prop:'#c9a6e0',sec:'#e0937e'};

function highlight(text,fileType){
    const ruleSet=rules[fileType];
    if(!ruleSet)return escape(text);

    // Build combined regex from all rules.
    const sources=ruleSet.map(([,pat])=>`(${pat.source})`);
    const combined=new RegExp(sources.join('|'),'gm');

    const parts=[];let lastIndex=0;
    let match;
    while((match=combined.exec(text))!==null){
        // Add text before match.
        if(match.index>lastIndex)parts.push(escape(text.slice(lastIndex,match.index)));

        // Find which group matched.
        let cls='';
        for(let i=0;i<ruleSet.length;i++){
            if(match[i+1]!==undefined){cls=ruleSet[i][0];break;}
        }

        const color=colors[cls];
        parts.push(color?`<span style="color:var(--syntax-${cls},${color})">${escape(match[0])}</span>`:escape(match[0]));
        lastIndex=match.index+match[0].length;

        // Prevent infinite loop on zero-length matches.
        if(match[0].length===0)combined.lastIndex++;
    }

    // Add remaining text.
    if(lastIndex<text.length)parts.push(escape(text.slice(lastIndex)));

    return parts.join('');
}

export function highlightSource(text,ext){
    // Map file extensions to rule sets.
    const typeMap={html:'html',htm:'html',svg:'html',xml:'html',css:'css',js:'js',mjs:'js',jsx:'js',ts:'js',tsx:'js',json:'json',toml:'toml'};
    let fileType=typeMap[ext];

    // Auto-detect if extension not recognized.
    if(!fileType){
        if(/^\s*[\{\[]/.test(text)&&/[\}\]]\s*$/.test(text))fileType='json';
        else if(/<\/?[a-zA-Z][\s\S]*>/.test(text))fileType='html';
        else if(/(?:function|const|let|var|import|export)\s/.test(text))fileType='js';
        else if(/(?:#[\da-fA-F]{3,8}|[\w-]+\s*:\s*[^;]+;)/.test(text))fileType='css';
        else return escape(text);
    }

    return highlight(text,fileType);
}
