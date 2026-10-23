import test from 'node:test';
import assert from 'node:assert/strict';
import {markdownHtml,staticDocument,textDocument} from '../file-content.js';
test('Markdown supports headings, emphasis and fences but never interprets raw HTML or links',()=>{
    const html=markdownHtml('# Hello\n**world**\n<script>alert(1)</script>\n[x](javascript:bad)\n```js\n<img src=x onerror=bad>\n```');
    assert.match(html,/<h1>Hello<\/h1>/);assert.match(html,/<strong>world<\/strong>/);assert.doesNotMatch(html,/<script|<img|<a\s/);assert.match(html,/&lt;script&gt;/);assert.match(html,/<pre><code>/);
    assert.match(staticDocument(html),/default-src &#39;none&#39;|default-src 'none'/);assert.doesNotMatch(textDocument('<script>bad</script>'),/<script>/);
});
