import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { lstatSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

export const TODO_BROWSER_DEVELOPMENT_ROOT = 'site/evidence/todo-browser-development';
const FILES = ['browser-after.png', 'browser-before.png', 'browser-card-after.png',
  'browser-full-after.png', 'index.html', 'long-list-browser-receipt.json',
  'long-list-terminal-summary.json', 'read-only-card-receipt.json', 'receipt.json',
  'todo-20-card.png', 'todo-20-full.png'];
const sha = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
const commit = value => /^[a-f0-9]{40}$/.test(value ?? '');

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
  const longList = JSON.parse(bytes('long-list-terminal-summary.json'));
  const longListBrowser = JSON.parse(bytes('long-list-browser-receipt.json'));
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
  if (longList.schema !== 'conduit.proof/todo-long-list-terminal@1'
      || longListBrowser.schema !== 'conduit.proof/todo-owner-browser-long-list@1'
      || longList.body_id !== action.body_id || longListBrowser.body_id !== action.body_id
      || !commit(longList.owner_source_commit)
      || longListBrowser.terminal_actions_source_identity !== longList.owner_source_commit
      || !commit(longListBrowser.owner_source_identity)
      || longListBrowser.owner_source_identity !== longListBrowser.browser_source_identity
      || longListBrowser.source_relation !== 'exact-source'
      || longListBrowser.open !== 3 || longListBrowser.completed !== 17
      || longListBrowser.item_count !== 20 || longList.open !== 3
      || longList.completed !== 17 || longList.status !== '3 things left · 17 completed'
      || longList.action_count !== 19
      || !Array.isArray(longList.items) || longList.items.length !== 20
      || longList.items.filter(item => item.complete === true).length !== 17
      || longList.items.filter(item => item.complete === false).length !== 3
      || !longListBrowser.browser_face_id
      || Number(longListBrowser.browser_face_revision) <= Number(longList.face_revision)
      || !Array.isArray(longListBrowser.errors) || longListBrowser.errors.length !== 0) {
    throw new Error('Todo long-list evidence lacks one matching Body and exact browser rejoin');
  }
  for (const source of [action.owner_source_commit, action.browser_runtime_source_commit,
    longList.owner_source_commit, longListBrowser.owner_source_identity]) {
    try { execFileSync('git', ['merge-base', '--is-ancestor', source, publicationCommit]); }
    catch { throw new Error('Todo browser capture source is absent from publication ancestry'); }
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
    if (longListBrowser.screenshots?.[name] && longListBrowser.screenshots[name] !== sha(image)) {
      throw new Error(`Todo browser long-list capture differs from its receipt: ${name}`);
    }
  }
  if (action.screenshots?.join(',') !== 'browser-before.png,browser-after.png'
      || !card.screenshots?.['browser-card-after.png']
      || !card.screenshots?.['browser-full-after.png']
      || !longListBrowser.screenshots?.['todo-20-card.png']
      || !longListBrowser.screenshots?.['todo-20-full.png']) {
    throw new Error('Todo browser receipts omit captured images');
  }
  const page = bytes('index.html').toString('utf8');
  if (!page.includes('cross-source development evidence')
      || !page.includes('This run proves one browser Add action')
      || !page.includes(action.owner_source_commit.slice(0, 9))
      || !page.includes(action.browser_runtime_source_commit.slice(0, 9))
      || !page.includes(longList.owner_source_commit.slice(0, 9))
      || !page.includes(longListBrowser.owner_source_identity.slice(0, 9))) {
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
    browserRuntimeCommit: action.browser_runtime_source_commit, bodyId: action.body_id,
    longListActionsCommit: longList.owner_source_commit,
    longListBrowserCommit: longListBrowser.owner_source_identity };
}
