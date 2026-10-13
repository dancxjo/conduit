// Verify caller-supplied installed owner, packaged UI/runtime and pinned browser.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export async function suppliedTodoBrowserProducts(args) {
  const [ownerCwdArg, binaryArg, stateArg, handbookArg, outputArg, playwrightArg,
    itemText = 'Pick up prescription', scenario = 'first-add'] = args;
  assert.ok(['first-add', 'cross-mask', 'cross-mask-speech', 'cross-mask-speech-resume', 'look-replacement', 'retained-dom-completion', 'correspondence', 'retained-correspondence'].includes(scenario), 'unknown Todo capture scenario');
  const retainedDom = scenario === 'retained-dom-completion';
  const correspondence = scenario === 'correspondence' || scenario === 'retained-correspondence';
  const resumeExisting = scenario === 'cross-mask-speech-resume' || retainedDom || scenario === 'retained-correspondence';
  const captureSpeech = scenario.startsWith('cross-mask-speech');
  if (!ownerCwdArg || !binaryArg || !stateArg || !handbookArg || !outputArg || !playwrightArg) {
    throw new Error('usage: todo-owner-browser.mjs OWNER-CWD INSTALLED-OWNER OWNER-STATE HANDBOOK-PACKAGE NEW-EVIDENCE-DIR PINNED-PLAYWRIGHT [NEW-ITEM-TEXT] [first-add|cross-mask|cross-mask-speech|cross-mask-speech-resume|look-replacement|retained-dom-completion|correspondence|retained-correspondence]');
  }
  const ownerCwd = path.resolve(ownerCwdArg);
  const binary = path.resolve(ownerCwd, binaryArg);
  const state = path.resolve(ownerCwd, stateArg);
  const handbook = path.resolve(handbookArg);
  const output = path.resolve(outputArg);
  assert.equal(existsSync(output), false, 'Todo evidence directory must be new');
  assert.ok(itemText && Buffer.byteLength(itemText, 'utf8') <= 64, 'Todo text must fit its admitted bound');
  const installed = JSON.parse(await readFile(path.join(state, 'installation.json')));
  assert.equal(path.resolve(ownerCwd, installed.product_executable), binary,
    'use the owner installed for this Body');
  if (captureSpeech) assert.equal(installed.selected_speech?.artifact_only, true,
    'direct speech capture requires explicit selected artifact output');
  const packageManifest = JSON.parse(await readFile(path.join(handbook, 'application.application.json')));
  const browserBundle = JSON.parse(await readFile(path.join(handbook, 'sdk/bundle/browser-bundle-release.json')));
  const uiResource = packageManifest.resources.find(resource => resource.role === 'owner-participation');
  const digest = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
  assert.equal(digest(await readFile(path.join(ownerCwd, 'targets/browser/handbook/owner-participation.mjs'))),
    uiResource.sha256, 'packaged Handbook UI differs from this source checkout');
  const head = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: ownerCwd, encoding: 'utf8' });
  assert.equal(head.status, 0, head.stderr);
  const handbookUiSource = head.stdout.trim();
  const uiStatus = spawnSync('git', ['status', '--porcelain', '--', 'targets/browser/handbook'],
    { cwd: ownerCwd, encoding: 'utf8' });
  assert.equal(uiStatus.status, 0, uiStatus.stderr);
  const handbookUiSourceClean = uiStatus.stdout.length === 0;
  const sourceRecord = {
    owner_source_commit: installed.release_source_identity,
    browser_runtime_source_commit: browserBundle.source_identity,
    handbook_ui_source_commit: handbookUiSource,
    handbook_ui_source_clean: handbookUiSourceClean,
    handbook_package_digest: packageManifest.package_digest,
    source_relation: installed.release_source_identity === browserBundle.source_identity
      && installed.release_source_identity === handbookUiSource && handbookUiSourceClean
      ? 'exact-source' : 'development-cross-source',
  };
  if (retainedDom || correspondence) assert.equal(sourceRecord.source_relation, 'exact-source',
    'retained DOM mutation requires identical clean owner/runtime/UI source');
  const expectedPlaywright = JSON.parse(await readFile(new URL('./package.json', import.meta.url)))
    .devDependencies['@playwright/test'];
  const actualPlaywright = JSON.parse(await readFile(path.join(path.dirname(playwrightArg), 'package.json'))).version;
  assert.equal(actualPlaywright, expectedPlaywright, 'use pinned Playwright');
  const { chromium } = await import(pathToFileURL(playwrightArg));
  return { ownerCwd, binary, state, handbook, output, itemText, scenario, retainedDom,
    correspondence, resumeExisting, captureSpeech, sourceRecord, chromium };
}
