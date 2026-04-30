import { readdirSync, readFileSync, writeFileSync } from 'fs';
import { join } from 'path';
import { gzipSync, brotliCompressSync, constants } from 'zlib';

const distDir = new URL('../dist', import.meta.url).pathname;

function getAllFiles(dir) {
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...getAllFiles(full));
    } else if (entry.isFile()) {
      files.push(full);
    }
  }
  return files;
}

const files = getAllFiles(distDir);
let gzCount = 0;
let brCount = 0;

for (const file of files) {
  const name = file.toLowerCase();
  if (name.endsWith('.gz') || name.endsWith('.br')) continue;

  const data = readFileSync(file);

  try {
    const gzipped = gzipSync(data, { level: 6 });
    if (gzipped.length < data.length) {
      writeFileSync(file + '.gz', gzipped);
      gzCount++;
    }
  } catch {}

  try {
    const br = brotliCompressSync(data, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 6 },
    });
    if (br.length < data.length) {
      writeFileSync(file + '.br', br);
      brCount++;
    }
  } catch {}
}

console.log(`Compressed: ${gzCount} gzip + ${brCount} brotli = ${gzCount + brCount} total files`);
