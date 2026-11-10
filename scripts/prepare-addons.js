import { createHash } from 'node:crypto';
import { lstat, readdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  addonCatalogPath, addonRoot, checkAddonId, resourceManifest,
  validateAddonCatalog, validateAddonManifest,
} from '../src/addons.js';

const publicRoot = fileURLToPath(new URL('../public/', import.meta.url));
const compareUtf8 = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));

function canonicalJson(value) {
  if (Array.isArray(value)) {
    return '[' + value.map(canonicalJson).join(',') + ']';
  }
  if (value && typeof value === 'object') {
    const fields = Object.keys(value).sort(compareUtf8);
    return '{' + fields.map(key => JSON.stringify(key) + ':' + canonicalJson(value[key])).join(',') + '}';
  }
  return JSON.stringify(value);
}

async function listFiles(root, prefix = '') {
  const entries = await readdir(join(root, prefix), { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = prefix + entry.name;
    if (entry.isDirectory()) {
      files.push(...await listFiles(root, path + '/'));
    } else if (entry.isFile()) {
      files.push(path);
    } else {
      throw new Error(`${path}: ожидается обычный файл или папка`);
    }
  }
  return files;
}

function hashField(hash, bytes) {
  const length = Buffer.alloc(8);
  length.writeBigUInt64BE(BigInt(bytes.length));
  hash.update(length).update(bytes);
}

async function prepareAddon(folder, ids) {
  const name = addonRoot + '/' + folder;
  const root = join(publicRoot, name);
  try {
    if (!(await lstat(root)).isDirectory()) {
      throw new Error('ожидается папка аддона');
    }
    const files = await listFiles(root);
    if (!files.includes(resourceManifest)) {
      throw new Error(`отсутствует ${resourceManifest}`);
    }

    const manifestPath = join(root, resourceManifest);
    const original = await readFile(manifestPath, 'utf8');
    const manifest = validateAddonManifest(JSON.parse(original), { requireHash: false });
    checkAddonId(ids, manifest.id);
    for (const path of manifest.layers) {
      if (!files.includes(path)) {
        throw new Error(`отсутствует ${path}`);
      }
    }

    const { sha256, ...metadata } = manifest;
    const canonicalManifest = Buffer.from(canonicalJson(metadata));
    const hash = createHash('sha256');
    for (const path of files.sort(compareUtf8)) {
      hashField(hash, Buffer.from(path));
      const content = path === resourceManifest ? canonicalManifest : await readFile(join(root, path));
      hashField(hash, content);
    }
    const digest = hash.digest('hex');
    if (digest !== sha256) {
      await writeFile(manifestPath, JSON.stringify({ ...metadata, sha256: digest }, null, 2) + '\n');
    }
  } catch (error) {
    throw new Error(`${name}: ${error.message}`);
  }
}

async function readCatalog() {
  try {
    const contents = await readFile(join(publicRoot, addonCatalogPath), 'utf8');
    return validateAddonCatalog(JSON.parse(contents));
  } catch (error) {
    throw new Error(`${addonCatalogPath}: ${error.message}`);
  }
}

try {
  const ids = new Set();
  for (const folder of await readCatalog()) {
    await prepareAddon(folder, ids);
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
