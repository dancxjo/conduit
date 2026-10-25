import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { preparePublicationSite } from '../../tools/ci/pipeline/publication-site.mjs';

async function fixture(t, html) {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'publication-site-fixture-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  await mkdir(path.join(directory, 'browser/site'), { recursive: true });
  await writeFile(path.join(directory, 'browser/site/index.html'), html);
  return directory;
}

test('fresh site composition validates links before exposing output and preserves the sealed input', async t => {
  const directory = await fixture(t, '<a href="journey.html">Journey</a>');
  const source = 'a'.repeat(40);
  let site;
  const result = await preparePublicationSite({ directory, source, command(program, args, options) {
    if (args[0].endsWith('/stage-conduitos-pages-evidence.mjs')) {
      assert.equal(args[1], path.join(directory, 'conduitos-x86_64/evidence/journey-frames'));
      assert.equal(args[3], source);
      site = args[2];
      // Stage fixture content only; the real journey stager has its own refusal suite.
      return spawnSync(program, ['-e', 'require("node:fs").writeFileSync(process.argv[1], "<p>Journey</p>")',
        path.join(site, 'journey.html')], options);
    }
    return spawnSync(program, args, options); // Execute the actual link checker.
  } });
  t.after(() => rm(path.dirname(site), { recursive: true, force: true }));
  assert.equal(result, site);
  assert.notEqual(site, path.join(directory, 'browser/site'));
  assert.equal(await readFile(path.join(directory, 'browser/site/index.html'), 'utf8'), '<a href="journey.html">Journey</a>');
  await assert.rejects(readFile(path.join(directory, 'browser/site/journey.html')), { code: 'ENOENT' });
});

test('real link failure retains the fresh site and refuses output', async t => {
  const directory = await fixture(t, '<a href="missing.html">Absent</a>');
  let site;
  await assert.rejects(preparePublicationSite({ directory, source: 'a'.repeat(40), command(program, args, options) {
    if (args[0].endsWith('/stage-conduitos-pages-evidence.mjs')) {
      site = args[2];
      return { status: 0, stdout: '', stderr: '' };
    }
    return spawnSync(program, args, options);
  } }), /verify-site-links.mjs failed:.*broken local links/s);
  t.after(() => rm(path.dirname(site), { recursive: true, force: true }));
  assert.equal(await readFile(path.join(site, 'index.html'), 'utf8'), '<a href="missing.html">Absent</a>');
});
