import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
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
