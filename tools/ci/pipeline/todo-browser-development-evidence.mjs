import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import { lstatSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

export const TODO_BROWSER_DEVELOPMENT_ROOT = 'site/evidence/todo-browser-development';
const FILES = ['browser-after.png', 'browser-before.png', 'browser-card-after.png',
  'browser-full-after.png', 'index.html', 'read-only-card-receipt.json', 'receipt.json'];
const sha = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
const commit = value => /^[a-f0-9]{40}$/.test(value ?? '');

// Keep failure context limited to repository identities; never dump the environment.
function ancestryContext(source, publicationCommit) {
  const inspect = args => {
    const result = spawnSync('git', args, { encoding: 'utf8' });
    return { status: result.status, output: result.stdout?.trim(),
      error: result.error?.message || result.stderr?.trim() };
  };
  return JSON.stringify({ cwd: process.cwd(),
    version: inspect(['--version']),
    checkout: inspect(['rev-parse', '--show-toplevel', '--absolute-git-dir',
      '--is-shallow-repository', 'HEAD']),
    parents: inspect(['rev-list', '--parents', '--no-walk', source, publicationCommit]),
  });
}

/** Admit a retained partial journey only with its exact, separate source labels. */
export function retainedTodoBrowserDevelopmentEvidence(root = TODO_BROWSER_DEVELOPMENT_ROOT,
  publicationCommit) {
  const entry = lstatSync(root);
  if (!entry.isDirectory() || entry.isSymbolicLink()
      || readdirSync(root).sort().join(',') !== FILES.sort().join(',')) {
    throw new Error('Todo browser development evidence has unexpected files');
  }
  const bytes = name => {
    const file = path.join(root, name);
    const stat = lstatSync(file);
    if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`Invalid Todo evidence file: ${name}`);
    return readFileSync(file);
  };
  const actionBytes = bytes('receipt.json');
  const action = JSON.parse(actionBytes);
  const card = JSON.parse(bytes('read-only-card-receipt.json'));
  if (!commit(publicationCommit) || !commit(action.owner_source_commit)
      || !commit(action.browser_runtime_source_commit)
      || action.schema !== 'conduit.proof/todo-owner-browser@1'
      || card.schema !== 'conduit.proof/todo-owner-browser-read-only-card@1'
      || action.source_relation !== 'development-cross-source'
      || card.source_relation !== action.source_relation
      || action.owner_source_commit !== action.handbook_ui_source_commit
      || action.browser_runtime_source_commit === action.owner_source_commit
      || action.handbook_ui_source_clean !== true
      || card.owner_source_commit !== action.owner_source_commit
      || card.browser_runtime_source_commit !== action.browser_runtime_source_commit
      || card.handbook_ui_source_commit !== action.handbook_ui_source_commit
      || card.handbook_package_digest !== action.handbook_package_digest
      || card.body_id !== action.body_id || card.item_text !== action.item_text
      || !action.before?.show_id || !action.after?.show_id
      || action.before.show_id === action.after.show_id
      || Number(action.after.face_revision) <= Number(action.before.face_revision)
      || card.original_receipt_sha256 !== sha(actionBytes)) {
    throw new Error('Todo browser development receipts do not describe one truthful Add');
  }
  for (const source of [action.owner_source_commit, action.browser_runtime_source_commit]) {
    try { execFileSync('git', ['merge-base', '--is-ancestor', source, publicationCommit]); }
    catch (cause) {
      throw new Error(`Todo browser capture ancestry check failed: ${source} -> ${publicationCommit}; status=${cause.status ?? cause.code ?? "unknown"}, signal=${cause.signal ?? "none"}; ${cause.message}; context=${ancestryContext(source, publicationCommit)}`, { cause });
    }
  }
  for (const name of FILES.filter(file => file.endsWith('.png'))) {
    const image = bytes(name);
    if (image.length < 24 || !image.subarray(0, 8).equals(Buffer.from('89504e470d0a1a0a', 'hex'))
        || image.toString('ascii', 12, 16) !== 'IHDR') {
      throw new Error(`Todo browser capture is not a PNG: ${name}`);
    }
    if (card.screenshots?.[name] && card.screenshots[name] !== sha(image)) {
      throw new Error(`Todo browser card capture differs from its receipt: ${name}`);
    }
  }
  if (action.screenshots?.join(',') !== 'browser-before.png,browser-after.png'
      || !card.screenshots?.['browser-card-after.png']
      || !card.screenshots?.['browser-full-after.png']) {
    throw new Error('Todo browser receipts omit captured images');
  }
  const page = bytes('index.html').toString('utf8');
  if (!page.includes('cross-source development evidence')
      || !page.includes('This run proves one browser Add action')
      || !page.includes(action.owner_source_commit.slice(0, 9))
      || !page.includes(action.browser_runtime_source_commit.slice(0, 9))) {
    throw new Error('Todo browser development page overclaims its capture');
  }
  for (const [, reference] of page.matchAll(/\b(?:href|src)="([^"]+)"/g)) {
    if (!reference.startsWith('/conduit/') && !reference.startsWith('https://')
        && !reference.startsWith('#')
        && !FILES.includes(reference)) {
      throw new Error(`Todo browser development page links an undeclared asset: ${reference}`);
    }
  }
  return { root, sourceCommit: action.owner_source_commit,
    browserRuntimeCommit: action.browser_runtime_source_commit, bodyId: action.body_id };
}
