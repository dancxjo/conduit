import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { renderTickerIntroduction } from '../../tools/ci/pipeline/targets/site.mjs';
import { verifySiteLinks } from '../../tools/ci/verify-site-links.mjs';

function withSite(run) {
  const root = mkdtempSync(path.join(tmpdir(), 'conduit-site-links-'));
  const put = (name, content) => {
    const file = path.join(root, name);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, content);
  };
  try { run(root, put); }
  finally { rmSync(root, { recursive: true, force: true }); }
}

test('accepts local pages, media, anchors, and external links', () => withSite((root, put) => {
  put('index.html', '<p id="start">start</p><a href="journeys/">Journeys</a><a href="https://example.org/">External</a>');
  put('journeys/index.html', '<a href="/conduit/#start">Home</a><img src="shot.png">');
  put('journeys/shot.png', 'png fixture');
  assert.deepEqual(verifySiteLinks(root), { pages: 2, references: 3 });
}));

test('rejects missing media and fragment targets before publication', () => withSite((root, put) => {
  put('index.html', '<a href="journeys/#absent">Jump</a>');
  put('journeys/index.html', '<img src="missing.png"><p id="present">here</p>');
  assert.throws(() => verifySiteLinks(root), /#absent has no anchor[\s\S]*missing\.png is absent/);
}));

test('catches commit-addressed back links that stop at a directory without an index', () => withSite((root, put) => {
  put('index.html', '<main>Home</main>');
  put('journeys/index.html', '<main>Journeys</main>');
  put('journeys/commits/abc/three-bodies/index.html', '<a href="../../">Back</a>');
  assert.throws(() => verifySiteLinks(root), /\.\.\/\.\.\/ is absent/);
  put('journeys/commits/abc/three-bodies/index.html', '<a href="../../../">Back</a>');
  assert.equal(verifySiteLinks(root).references, 1);
}));

test('ticker entrance is truthful and preserves historical recording links and bytes', () => withSite((root, put) => {
  put('journeys/index.html', '<a href="current/ticker/">Ticker</a>');
  const recording = '<h1>One clock, three live Hosts</h1><a href="receipt.json">Original receipt</a>';
  put('journeys/current/one-body-five-masks/index.html', recording);
  put('journeys/current/one-body-five-masks/receipt.json', '{"original":true}');
  put('journeys/development/todo-browser/index.html', '<h1>Todo development steps</h1>');
  renderTickerIntroduction(path.join(root, 'journeys/current/ticker'));
  const page = readFileSync(path.join(root, 'journeys/current/ticker/index.html'), 'utf8');
  assert.ok(page.includes('<h1>Start, stop, and change a ticker</h1>'));
  assert.ok(page.includes('It does not tell time of day.'));
  assert.ok(page.includes('250, 500, 1000, or 2000 milliseconds'));
  assert.ok(page.includes('running, stopped, or waiting to start'));
  assert.ok(page.includes('<details><summary>Inspect the original recording and receipts</summary>'));
  assert.ok(page.includes('Todo development steps'));
  assert.equal(readFileSync(path.join(root, 'journeys/current/one-body-five-masks/index.html'), 'utf8'), recording);
  // Site chrome is shared with every published page.
  for (const route of ['index.html', 'handbook/index.html', 'workspace/index.html']) {
    put(route, '<main id="get-conduit">Site navigation destination</main>');
  }
  assert.ok(verifySiteLinks(root).references > 4);
}));
