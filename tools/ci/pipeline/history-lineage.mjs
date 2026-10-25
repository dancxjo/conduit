// Historical capture identity survives an explicit, source-preserving history
// migration. Live train/source admission continues to require actual ancestry.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, lstatSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { requireExactAncestor } from './exact-ancestor.mjs';

export const CAPTURE_LINEAGE_POLICY = 'docs/audits/history-capture-lineage.json';
const OID = /^[a-f0-9]{40}$/;
const HASH = /^[a-f0-9]{64}$/;
const MAX_POLICY = 4 * 1024 * 1024;
const fail = message => { throw new Error(`Capture lineage: ${message}`); };
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const objectId = (kind, bytes) => createHash('sha1').update(`${kind} ${bytes.length}\0`).update(bytes).digest('hex');
const pairBytes = pair => Buffer.from(JSON.stringify([pair.original, pair.mapped]));
const leaf = pair => hash(Buffer.concat([Buffer.from([0]), pairBytes(pair)]));
const node = (left, right) => hash(Buffer.concat([Buffer.from([1]), Buffer.from(left, 'hex'), Buffer.from(right, 'hex')]));
const countedRoot = (top, count) => {
  const size = Buffer.alloc(4); size.writeUInt32BE(count);
  return hash(Buffer.concat([Buffer.from([2]), size, Buffer.from(top, 'hex')]));
};

function requirePair(pair) {
  if (!OID.test(pair?.original ?? '') || !OID.test(pair?.mapped ?? '')) fail('noncanonical map identity');
}

/** A domain-separated Merkle root binds witnesses to the complete frozen map. */
export function historyMap(entries) {
  if (!Array.isArray(entries) || !entries.length || entries.length > 200000) fail('map count exceeds bound');
  const pairs = entries.map(pair => { requirePair(pair); return { original: pair.original, mapped: pair.mapped }; })
    .sort((a, b) => a.original.localeCompare(b.original));
  if (new Set(pairs.map(pair => pair.original)).size !== pairs.length) fail('duplicate original map identity');
  const levels = [pairs.map(leaf)];
  while (levels.at(-1).length > 1) {
    const prior = levels.at(-1), next = [];
    for (let i = 0; i < prior.length; i += 2) next.push(node(prior[i], prior[i + 1] ?? prior[i]));
    levels.push(next);
  }
  const positions = new Map(pairs.map((pair, i) => [pair.original, i]));
  return { root: countedRoot(levels.at(-1)[0], pairs.length), count: pairs.length, witness(original) {
    let index = positions.get(original);
    if (index === undefined) fail('capture or parent absent from frozen map');
    const pair = pairs[index], proof = [];
    for (const level of levels.slice(0, -1)) {
      proof.push({ side: index % 2 ? 'left' : 'right', hash: level[index ^ 1] ?? level[index] });
      index = Math.floor(index / 2);
    }
    return { ...pair, proof };
  } };
}

function verifyPair(pair, root, count) {
  requirePair(pair);
  const depth = Math.ceil(Math.log2(count));
  if (!Array.isArray(pair.proof) || pair.proof.length !== depth) fail('invalid map inclusion proof length');
  let current = leaf(pair);
  for (const sibling of pair.proof) {
    if (!HASH.test(sibling?.hash ?? '') || !['left', 'right'].includes(sibling.side)) fail('invalid map inclusion proof');
    current = sibling.side === 'left' ? node(sibling.hash, current) : node(current, sibling.hash);
  }
  if (countedRoot(current, count) !== root) fail('capture or parent is not in authenticated map');
}

function invokeGit(cwd, args, maxBuffer = 16 * 1024 * 1024) {
  const result = spawnSync('git', ['--no-replace-objects', '-c', 'core.commitGraph=false', ...args], {
    cwd, maxBuffer, timeout: 120000, env: { ...process.env, GIT_NO_LAZY_FETCH: '1' },
  });
  if (result.error || result.signal || result.status !== 0) fail(`Git refused ${args[0]}`);
  return result.stdout;
}

function parseCommit(raw) {
  if (!Buffer.isBuffer(raw) || raw.length > 256 * 1024) fail('commit witness exceeds bound');
  const end = raw.indexOf('\n\n');
  if (end < 0) fail('malformed original commit');
  const headers = raw.subarray(0, end).toString('latin1').split('\n');
  if (!/^tree [a-f0-9]{40}$/.test(headers[0])) fail('malformed original tree');
  const parents = [];
  let index = 1;
  while (headers[index]?.startsWith('parent ')) {
    const parent = headers[index++].slice(7);
    if (!OID.test(parent) || parents.length >= 32 || parents.includes(parent)) fail('malformed original parents');
    parents.push(parent);
  }
  const metadata = [];
  for (; index < headers.length; index++) {
    const line = headers[index];
    if (/^(tree|parent|mergetag)( |$)/.test(line)) fail('unsupported original commit header');
    // Original signatures remain in the original witness/backup. Rewritten
    // commits cannot claim those signatures authenticate their changed parents.
    if (/^gpgsig(?:-sha256)? /.test(line)) {
      while (headers[index + 1]?.startsWith(' ')) index++;
    } else metadata.push(line);
  }
  if (!metadata.some(line => line.startsWith('author ')) || !metadata.some(line => line.startsWith('committer '))) fail('missing original authorship');
  return { tree: headers[0].slice(5), parents, metadata,
    message: raw.subarray(end + 2) };
}

function asset(asset, maxBytes) {
  if (!Number.isSafeInteger(asset?.releaseId) || asset.releaseId <= 0 ||
      !Number.isSafeInteger(asset?.assetId) || asset.assetId <= 0 ||
      !Number.isSafeInteger(asset?.bytes) || asset.bytes <= 0 || asset.bytes > maxBytes ||
      !HASH.test(asset?.sha256 ?? '') || typeof asset.url !== 'string') fail('invalid immutable custody locator');
  const url = new URL(asset.url);
  if (url.origin !== 'https://github.com' || url.search || url.hash || url.username || url.password ||
      !/^\/dancxjo\/conduit-evidence\/releases\/download\/[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+$/.test(url.pathname)) fail('invalid immutable custody URL');
}

function validatePolicy(policy) {
  if (policy?.schema !== 'conduit.capture-history-lineage/v1' || policy.repository !== 'dancxjo/conduit' ||
      !HASH.test(policy.map?.root ?? '') || !Number.isSafeInteger(policy.map.count) || policy.map.count < 1 || policy.map.count > 200000 ||
      !Array.isArray(policy.captures) || policy.captures.length < 1 || policy.captures.length > 128 ||
      new Set(policy.captures.map(capture => capture.original)).size !== policy.captures.length) fail('invalid committed migration policy');
  asset(policy.map.asset, 64 * 1024 * 1024);
  asset(policy.originalBackup, 2 * 1024 ** 3);
  for (const capture of policy.captures) readOriginalWitness(policy, capture);
}

/** Offline construction; caller must first independently verify immutable asset
 * custody and complete final ref/blob/map preservation. No refs are changed. */
export function createCaptureLineagePolicy({ mapBytes, mapAsset, originalBackup,
  captures, originalDirectory, mappedDirectory }) {
  if (!Buffer.isBuffer(mapBytes) || mapBytes.length !== mapAsset.bytes || hash(mapBytes) !== mapAsset.sha256) fail('map asset differs from frozen bytes');
  if (!Buffer.from(mapBytes.toString('ascii'), 'ascii').equals(mapBytes)) fail('frozen map must be ASCII');
  const lines = mapBytes.toString('ascii').trimEnd().split('\n');
  if (!/^old\s+new$/.test(lines.shift() ?? '')) fail('invalid frozen map header');
  const entries = lines.map(line => {
    if (!/^[a-f0-9]{40} [a-f0-9]{40}$/.test(line)) fail('invalid frozen map row');
    const [original, mapped] = line.split(' ');
    if (mapped === '0'.repeat(40)) fail('capture lineage requires preserved commit topology');
    return { original, mapped };
  });
  if (!Array.isArray(captures) || captures.length < 1 || captures.length > 128 || captures.some(original => !OID.test(original ?? ''))) fail('bounded full capture commit IDs required');
  const merkle = historyMap(entries);
  const policy = { schema: 'conduit.capture-history-lineage/v1', repository: 'dancxjo/conduit',
    map: { root: merkle.root, count: merkle.count, asset: mapAsset }, originalBackup,
    captures: captures.map(original => {
      const raw = invokeGit(originalDirectory, ['cat-file', 'commit', original], 256 * 1024);
      if (objectId('commit', raw) !== original) fail('original object hash differs');
      return { ...merkle.witness(original), originalCommit: raw.toString('base64'),
        parents: parseCommit(raw).parents.map(parent => merkle.witness(parent)) };
    }) };
  validatePolicy(policy);
  for (const capture of policy.captures) verifyCaptureWitness(policy, capture, mappedDirectory);
  if (Buffer.byteLength(JSON.stringify(policy, null, 2) + '\n') > MAX_POLICY) fail('compact policy exceeds bound');
  return policy;
}

function readOriginalWitness(policy, capture) {
  verifyPair(capture, policy.map.root, policy.map.count);
  if (typeof capture.originalCommit !== 'string' || capture.originalCommit.length > 350000) fail('invalid original commit witness');
  const raw = Buffer.from(capture.originalCommit, 'base64');
  if (raw.toString('base64') !== capture.originalCommit || objectId('commit', raw) !== capture.original) fail('original commit witness hash differs');
  const original = parseCommit(raw);
  if (!Array.isArray(capture.parents) || capture.parents.length !== original.parents.length) fail('original parent inventory differs');
  for (const [index, parent] of capture.parents.entries()) {
    if (parent.original !== original.parents[index]) fail('original parent order differs');
    verifyPair(parent, policy.map.root, policy.map.count);
  }
  return original;
}

function verifyCaptureWitness(policy, capture, cwd) {
  const original = readOriginalWitness(policy, capture);
  const mapped = invokeGit(cwd, ['cat-file', 'commit', capture.mapped], 256 * 1024);
  const translated = Buffer.concat([Buffer.from([
    `tree ${original.tree}`, ...capture.parents.map(parent => `parent ${parent.mapped}`), ...original.metadata,
  ].join('\n') + '\n\n', 'latin1'), original.message]);
  if (objectId('commit', mapped) !== capture.mapped || !mapped.equals(translated)) fail('mapped source changes tree, topology, authorship or message');
  const tree = invokeGit(cwd, ['cat-file', 'tree', original.tree]);
  if (objectId('tree', tree) !== original.tree) fail('mapped source tree object differs');
  // A missing nested tree must not be hidden behind matching root OIDs.
  invokeGit(cwd, ['ls-tree', '-r', '-t', '-z', original.tree]);
}

/** Only a policy committed at HEAD may bridge a historical capture. Runtime
 * callers cannot supply an unsigned map, a replacement ref or an environment
 * override. Map Merkle inclusion and raw commit witnesses are checked locally. */
export function requireCaptureLineage(source, publication, { cwd = process.cwd() } = {}) {
  if (!OID.test(source ?? '') || !OID.test(publication ?? '')) fail('full original and publication commit IDs required');
  const listing = invokeGit(cwd, ['ls-tree', '-z', 'HEAD', '--', CAPTURE_LINEAGE_POLICY], 4096);
  const policyFile = path.join(cwd, CAPTURE_LINEAGE_POLICY);
  if (!listing.length) {
    if (existsSync(policyFile)) fail('uncommitted migration policy');
    requireExactAncestor(source, publication, { cwd });
    return { relation: 'actual-ancestry', captureSource: source, publicationSource: publication };
  }
  if (!/^100644 blob [a-f0-9]{40}\tdocs\/audits\/history-capture-lineage\.json\0$/.test(listing.toString('utf8'))) fail('committed migration policy must be a regular blob');
  requireExactAncestor(publication, publication, { cwd });
  if (invokeGit(cwd, ['rev-parse', 'HEAD'], 4096).toString().trim() !== publication) fail('policy checkout must equal publication source');
  const info = lstatSync(policyFile);
  if (!info.isFile() || info.isSymbolicLink() || info.size > MAX_POLICY) fail('unsafe committed migration policy');
  const bytes = invokeGit(cwd, ['show', `HEAD:${CAPTURE_LINEAGE_POLICY}`], MAX_POLICY);
  if (!readFileSync(policyFile).equals(bytes)) fail('migration policy differs from committed source');
  const policy = JSON.parse(bytes);
  validatePolicy(policy);
  const capture = policy.captures.find(value => value.original === source);
  if (!capture) {
    requireExactAncestor(source, publication, { cwd });
    return { relation: 'actual-ancestry', captureSource: source, publicationSource: publication };
  }
  verifyCaptureWitness(policy, capture, cwd);
  requireExactAncestor(capture.mapped, publication, { cwd });
  return { relation: 'mapped-history', captureSource: source, mappedCaptureSource: capture.mapped,
    publicationSource: publication, policySha256: hash(bytes), mapRoot: policy.map.root,
    mapAsset: policy.map.asset, originalBackup: policy.originalBackup };
}
