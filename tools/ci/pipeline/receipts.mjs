import { createHash, randomUUID } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { lstat, readdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';

const SCHEMA = 'conduit.pipeline-target/v1';
const PROOF_CLASSES = new Set(['build', 'executable', 'browser', 'emulator']);

function requireIdentity(target, sha, proofClass) {
  if (typeof target !== 'string' || !/^[a-z0-9][a-z0-9_-]*$/.test(target)) {
    throw new Error(`Invalid target identity: ${target}`);
  }
  if (typeof sha !== 'string' || !/^[a-f0-9]{40}$/.test(sha)) {
    throw new Error('A full lowercase commit SHA is required');
  }
  if (!PROOF_CLASSES.has(proofClass)) throw new Error(`Unknown proof class: ${proofClass}`);
}

async function requireDirectory(directory) {
  const stat = await lstat(directory);
  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error(`Expected a real directory: ${directory}`);
  }
}

async function productFiles(directory, prefix = '') {
  await requireDirectory(directory);
  const files = [];
  for (const name of (await readdir(directory)).sort()) {
    // POSIX paths are the portable receipt identity, including on Windows.
    if (name.includes('\\')) throw new Error(`Nonportable product path: ${name}`);
    const fullPath = path.join(directory, name);
    const relativePath = prefix ? `${prefix}/${name}` : name;
    const stat = await lstat(fullPath);
    if (stat.isSymbolicLink()) throw new Error(`Symlink is not a product: ${relativePath}`);
    if (!prefix && name === 'receipt.json') {
      if (!stat.isFile()) throw new Error('receipt.json must be a regular file');
      continue;
    }
    if (stat.isDirectory()) {
      files.push(...await productFiles(fullPath, relativePath));
    } else if (stat.isFile()) {
      const hash = createHash('sha256');
      let size = 0;
      for await (const chunk of createReadStream(fullPath)) {
        hash.update(chunk);
        size += chunk.length;
      }
      files.push({ path: relativePath, size, sha256: hash.digest('hex') });
    } else {
      throw new Error(`Unsupported product file: ${relativePath}`);
    }
  }
  return files.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
}

// The target runner calls this only after its build and proof commands succeed.
// There is deliberately no command-line interface for standalone attestation.
export async function sealTarget({ directory, target, sha, proofClass }) {
  requireIdentity(target, sha, proofClass);
  const files = await productFiles(directory);
  if (!files.length) throw new Error(`No products for target ${target}`);
  const receipt = { schema: SCHEMA, target, sha, proofClass, files };
  const temporary = path.join(directory, `.receipt-${randomUUID()}.tmp`);
  try {
    await writeFile(temporary, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
    await rename(temporary, path.join(directory, 'receipt.json'));
  } finally {
    await rm(temporary, { force: true });
  }
  return receipt;
}

function validateFiles(files) {
  if (!Array.isArray(files) || !files.length) throw new Error('Receipt has no products');
  const seen = new Set();
  for (const file of files) {
    if (!file || typeof file.path !== 'string' || file.path.includes('\\') ||
        file.path.split('/').some(part => !part || part === '.' || part === '..') ||
        /^[A-Za-z]:/.test(file.path) || file.path === 'receipt.json') {
      throw new Error('Unsafe receipt product path');
    }
    if (seen.has(file.path)) throw new Error(`Duplicate receipt product: ${file.path}`);
    seen.add(file.path);
    if (!Number.isSafeInteger(file.size) || file.size < 0 ||
        typeof file.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(file.sha256)) {
      throw new Error(`Invalid product digest: ${file.path}`);
    }
  }
}

export async function verifyBundle({ directory, sha, targets }) {
  if (!Array.isArray(targets) || !targets.length) throw new Error('Required targets are empty');
  const expected = new Map();
  for (const { id, proofClass } of targets) {
    requireIdentity(id, sha, proofClass);
    if (expected.has(id)) throw new Error(`Duplicate required target: ${id}`);
    expected.set(id, proofClass);
  }
  await requireDirectory(directory);
  const entries = (await readdir(directory)).sort();
  if (entries.length !== expected.size || entries.some(name => !expected.has(name))) {
    throw new Error('Bundle target set differs from required target set');
  }
  const receipts = [];
  for (const target of entries) {
    const targetDirectory = path.join(directory, target);
    await requireDirectory(targetDirectory);
    const receiptPath = path.join(targetDirectory, 'receipt.json');
    const stat = await lstat(receiptPath);
    if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`Invalid receipt for ${target}`);
    const receipt = JSON.parse(await readFile(receiptPath, 'utf8'));
    if (receipt?.schema !== SCHEMA || receipt.target !== target || receipt.sha !== sha ||
        receipt.proofClass !== expected.get(target)) {
      throw new Error(`Receipt identity mismatch for ${target}`);
    }
    validateFiles(receipt.files);
    const actual = await productFiles(targetDirectory);
    const declared = receipt.files.map(({ path, size, sha256 }) => ({ path, size, sha256 }))
      .sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
    if (JSON.stringify(actual) !== JSON.stringify(declared)) {
      throw new Error(`Product digest or file set mismatch for ${target}`);
    }
    receipts.push(receipt);
  }
  return receipts;
}
