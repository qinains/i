import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';

const PORT = Number(process.env.PORT ?? 8080);
const ROOT = path.resolve(import.meta.dirname || '.');

const MIME = {
    '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript',
    '.css': 'text/css', '.json': 'application/json', '.png': 'image/png',
    '.jpg': 'image/jpeg', '.gif': 'image/gif', '.svg': 'image/svg+xml',
    '.ico': 'image/x-icon', '.webmanifest': 'application/manifest+json',
    '.woff': 'font/woff', '.woff2': 'font/woff2', '.txt': 'text/plain',
    '.webp': 'image/webp',
    '.wasm': 'application/wasm',
    '.zip': 'application/zip',
};

const server = http.createServer((req, res) => {
    let file = path.join(ROOT, req.url === '/' ? 'index.html' : req.url);
    if (!file.startsWith(ROOT)) { res.writeHead(403); res.end(); return; }
    fs.stat(file, (err, stat) => {
        if (err || !stat.isFile()) { res.writeHead(404); res.end('Not found'); return; }
        const ext = path.extname(file).toLowerCase();
        res.writeHead(200, {
            'Content-Type': MIME[ext] || 'application/octet-stream',
            'Cache-Control': 'no-cache',
        });
        fs.createReadStream(file).pipe(res);
    });
});
server.listen(PORT, '127.0.0.1', () => console.log(`Serving web/ on http://127.0.0.1:${server.address().port}`));
