/**
 * Fileus Web Frontend Server
 * Pure Bun/SolidJS HTTP server with file operations
 */
import http from 'http';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');
const distDir = path.join(rootDir, 'dist');

const HTTP_PORT = parseInt(process.env.PORT || '8080', 10);
const TAURI_HOST = process.env.TAURI_HOST || 'localhost';
const TAURI_PORT = parseInt(process.env.TAURI_PORT || '8081', 10);

const MIME = {
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.mjs': 'application/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
  '.wasm': 'application/wasm',
};

function sendJson(res, status, data) {
  res.statusCode = status;
  res.setHeader('Content-Type', 'application/json');
  res.end(JSON.stringify(data));
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    let body = '';
    req.on('data', chunk => body += chunk);
    req.on('end', () => resolve(body));
    req.on('error', reject);
  });
}

function proxyToTauri(req, res, tauriPath) {
  const tauriUrl = `http://${TAURI_HOST}:${TAURI_PORT}${tauriPath}`;
  const proxyReq = http.request(tauriUrl, { method: req.method, headers: req.headers }, (proxyRes) => {
    res.statusCode = proxyRes.statusCode || 200;
    Object.entries(proxyRes.headers).forEach(([k, v]) => res.setHeader(k, v));
    proxyRes.pipe(res);
  });
  proxyReq.on('error', () => {
    res.statusCode = 502;
    res.end(JSON.stringify({ error: 'Tauri backend unavailable' }));
  });
  req.pipe(proxyReq);
}

async function handleApi(req, res, pathname, reqUrl) {
  const baseRoute = '/api/file';

  if (pathname === '/api/health' && req.method === 'GET') {
    sendJson(res, 200, { status: 'ok', timestamp: Date.now() });
    return true;
  }

  if (pathname === '/api/greet' && req.method === 'GET') {
    const name = reqUrl.searchParams.get('name') || 'World';
    sendJson(res, 200, { message: `Hello, ${name}! (from Fileus HTTP server)` });
    return true;
  }

  if (pathname.startsWith('/api/files/list') && req.method === 'GET') {
    const dir = reqUrl.searchParams.get('dir') || '/';
    const safeDir = dir.includes('..') ? '/' : dir;
    try {
      const items = [];
      const entries = fs.readdirSync(safeDir, { withFileTypes: true });
      for (const entry of entries) {
        const fullPath = path.join(safeDir, entry.name);
        let stat;
        try { stat = fs.statSync(fullPath); } catch { continue; }
        items.push({
          name: entry.name,
          path: fullPath,
          is_dir: entry.isDirectory(),
          size: stat.size,
          modified: Math.floor(stat.mtimeMs / 1000),
        });
      }
      items.sort((a, b) => {
        if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      sendJson(res, 200, items);
    } catch (e) {
      sendJson(res, 500, { error: `Cannot read directory: ${e.message}` });
    }
    return true;
  }

  if (pathname === '/api/files/read' && req.method === 'GET') {
    const filePath = reqUrl.searchParams.get('path') || '';
    if (filePath.includes('..')) { sendJson(res, 403, { error: 'Forbidden' }); return true; }
    try {
      const content = fs.readFileSync(filePath, 'utf-8');
      sendJson(res, 200, { content });
    } catch (e) {
      sendJson(res, 500, { error: `Cannot read file: ${e.message}` });
    }
    return true;
  }

  if (pathname === '/api/files/write' && req.method === 'POST') {
    try {
      const body = JSON.parse(await readBody(req));
      const filePath = body.path || '';
      const content = body.content || '';
      if (filePath.includes('..')) { sendJson(res, 403, { error: 'Forbidden' }); return true; }
      fs.writeFileSync(filePath, content);
      sendJson(res, 200, { success: true });
    } catch (e) {
      sendJson(res, 500, { error: `Cannot write file: ${e.message}` });
    }
    return true;
  }

  if (pathname === '/api/files/create-dir' && req.method === 'POST') {
    try {
      const body = JSON.parse(await readBody(req));
      const dirPath = body.path || '';
      if (dirPath.includes('..')) { sendJson(res, 403, { error: 'Forbidden' }); return true; }
      fs.mkdirSync(dirPath, { recursive: true });
      sendJson(res, 200, { success: true });
    } catch (e) {
      sendJson(res, 500, { error: `Cannot create directory: ${e.message}` });
    }
    return true;
  }

  if (pathname === '/api/files/delete' && req.method === 'DELETE') {
    const filePath = reqUrl.searchParams.get('path') || '';
    if (filePath.includes('..') || filePath === '/') { sendJson(res, 403, { error: 'Forbidden' }); return true; }
    try {
      const stat = fs.statSync(filePath);
      if (stat.isDirectory()) fs.rmSync(filePath, { recursive: true });
      else fs.unlinkSync(filePath);
      sendJson(res, 200, { success: true });
    } catch (e) {
      sendJson(res, 500, { error: `Cannot delete: ${e.message}` });
    }
    return true;
  }

  if (pathname === '/api/files/rename' && req.method === 'POST') {
    try {
      const body = JSON.parse(await readBody(req));
      const oldPath = body.old_path || '';
      const newPath = body.new_path || '';
      if (oldPath.includes('..') || newPath.includes('..')) { sendJson(res, 403, { error: 'Forbidden' }); return true; }
      fs.renameSync(oldPath, newPath);
      sendJson(res, 200, { success: true });
    } catch (e) {
      sendJson(res, 500, { error: `Cannot rename: ${e.message}` });
    }
    return true;
  }

  if (pathname === '/api/files/copy' && req.method === 'POST') {
    try {
      const body = JSON.parse(await readBody(req));
      const source = body.source || '';
      const dest = body.dest || '';
      if (source.includes('..') || dest.includes('..')) { sendJson(res, 403, { error: 'Forbidden' }); return true; }
      fs.copyFileSync(source, dest);
      sendJson(res, 200, { success: true });
    } catch (e) {
      sendJson(res, 500, { error: `Cannot copy: ${e.message}` });
    }
    return true;
  }

  if (pathname === '/api/files/move' && req.method === 'POST') {
    try {
      const body = JSON.parse(await readBody(req));
      const source = body.source || '';
      const dest = body.dest || '';
      if (source.includes('..') || dest.includes('..')) { sendJson(res, 403, { error: 'Forbidden' }); return true; }
      fs.renameSync(source, dest);
      sendJson(res, 200, { success: true });
    } catch (e) {
      sendJson(res, 500, { error: `Cannot move: ${e.message}` });
    }
    return true;
  }

  if (pathname === '/api/files/search' && req.method === 'GET') {
    const dir = reqUrl.searchParams.get('dir') || '/';
    const pattern = reqUrl.searchParams.get('pattern') || '';
    if (dir.includes('..')) { sendJson(res, 403, { error: 'Forbidden' }); return true; }
    const results = [];
    const patternLower = pattern.toLowerCase();
    function walk(d) {
      try {
        const entries = fs.readdirSync(d, { withFileTypes: true });
        for (const entry of entries) {
          const fullPath = path.join(d, entry.name);
          if (entry.name.toLowerCase().includes(patternLower)) {
            try {
              const stat = fs.statSync(fullPath);
              results.push({
                name: entry.name,
                path: fullPath,
                is_dir: entry.isDirectory(),
                size: stat.size,
                modified: Math.floor(stat.mtimeMs / 1000),
              });
            } catch {}
          }
          if (entry.isDirectory()) walk(fullPath);
        }
      } catch {}
    }
    walk(dir);
    sendJson(res, 200, results);
    return true;
  }

  if (pathname === '/api/files/info' && req.method === 'GET') {
    const filePath = reqUrl.searchParams.get('path') || '';
    if (filePath.includes('..')) { sendJson(res, 403, { error: 'Forbidden' }); return true; }
    try {
      const stat = fs.statSync(filePath);
      sendJson(res, 200, {
        name: path.basename(filePath),
        path: filePath,
        is_dir: stat.isDirectory(),
        size: stat.size,
        modified: Math.floor(stat.mtimeMs / 1000),
      });
    } catch (e) {
      sendJson(res, 500, { error: `Cannot get info: ${e.message}` });
    }
    return true;
  }

  if (pathname === '/api/system/drives' && req.method === 'GET') {
    const drives = [];
    drives.push({ name: '/', path: '/', is_dir: true, size: 0, modified: null });
    const home = process.env.HOME || '/home/' + process.env.USER;
    if (home) drives.push({ name: 'Home', path: home, is_dir: true, size: 0, modified: null });
    drives.push({ name: 'Temp', path: '/tmp', is_dir: true, size: 0, modified: null });
    sendJson(res, 200, drives);
    return true;
  }

  if (pathname === '/api/system/home' && req.method === 'GET') {
    const home = process.env.HOME || '/home/' + process.env.USER;
    sendJson(res, 200, { path: home || '/' });
    return true;
  }

  return false;
}

function serveFile(req, res, filepath) {
  fs.readFile(filepath, (err, data) => {
    if (err) {
      res.statusCode = 404;
      res.end('Not found');
      return;
    }
    const ext = path.extname(filepath);
    const mime = MIME[ext] || 'application/octet-stream';
    res.setHeader('Content-Type', mime);
    res.setHeader('Cache-Control', 'no-cache');
    res.end(data);
  });
}

function createAppServer() {
  return (req, res) => {
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, DELETE, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

    if (req.method === 'OPTIONS') {
      res.statusCode = 204;
      res.end();
      return;
    }

    try {
      const reqUrl = new URL(req.url, `http://${req.headers.host}`);
      let safePath = decodeURIComponent(reqUrl.pathname);

      if (safePath.includes('..')) {
        res.statusCode = 403;
        res.end('Forbidden');
        return;
      }

      if (safePath.startsWith('/api/')) {
        handleApi(req, res, safePath, reqUrl).then(handled => {
          if (!handled) {
            res.statusCode = 404;
            res.end(JSON.stringify({ error: 'API not found' }));
          }
        });
        return;
      }

      if (safePath === '/' || safePath === '') {
        serveFile(req, res, path.join(distDir, 'index.html'));
        return;
      }

      const filepath = path.join(distDir, safePath);
      fs.stat(filepath, (err, stats) => {
        if (!err && stats.isFile()) {
          serveFile(req, res, filepath);
        } else {
          const indexPath = path.join(distDir, 'index.html');
          fs.stat(indexPath, (ie, istats) => {
            if (!ie && istats.isFile()) {
              serveFile(req, res, indexPath);
            } else {
              res.statusCode = 404;
              res.end('Not found');
            }
          });
        }
      });
    } catch (err) {
      res.statusCode = 400;
      res.end('Bad request');
    }
  };
}

console.log('=== Fileus Web Frontend Server ===');
console.log(`HTTP server: http://localhost:${HTTP_PORT}`);
console.log(`Tauri backend: http://${TAURI_HOST}:${TAURI_PORT}`);
console.log('Press Ctrl+C to stop.\n');

const httpServer = http.createServer(createAppServer());
httpServer.listen(HTTP_PORT);
