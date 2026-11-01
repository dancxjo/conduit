import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const root = fileURLToPath(new URL('../../', import.meta.url));
test('Workspace staging retains the actual split owner renderer and its closed package graph', async () => {
  const scratch = await mkdtemp(join(tmpdir(), 'conduit-workspace-owner-'));
  try {
    const release = join(scratch, 'release'), bin = join(scratch, 'bin');
    await mkdir(release); await mkdir(bin);
    // Envelope fixtures only: this test does not execute a runtime, release or Body.
    const runtime = Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]);
    for (const name of ['browser-page.json', 'runtime.wasm', 'index.html', 'host.mjs',
      'browser-host-bootstrap.mjs', 'browser-host-membership.mjs', 'browser-host-identity.mjs',
      'browser-boot-profile.mjs', 'browser-relay-line.mjs', 'media-host.mjs', 'device-base.mjs', 'usb-device-base.mjs']) {
      await writeFile(join(release, name), name === 'runtime.wasm' ? runtime : name.endsWith('.json') ? '{}' : 'fixture');
    }
    await writeFile(join(release, 'release-catalog.json'), JSON.stringify({ generation: 1 }));
    const body = join(scratch, 'body.conduit'), catalogue = join(scratch, 'catalogue.json');
    await writeFile(body, 'fixture'); await writeFile(catalogue, '{}');
    // Isolate only the auxiliary catalogue command; all source copying and module
    // dependency/byte checks run through the actual production stager and builder.
    const shim = '#!/bin/sh\nset -eu\ntest "$1 $2 $3 $4" = "xtask make host release-catalog"\nshift 4\ntest "$1" = "--root"\nmkdir -p "$2"\nprintf \'{"generation":1}\\n\' > "$2/release-catalog.json"\n';
    await writeFile(join(bin, 'cargo'), shim, { mode: 0o755 });
    const destination = join(scratch, 'workspace');
    const staged = spawnSync('sh', ['targets/browser/tools/stage-browser-workspace.sh',
      join(release, 'runtime.wasm'), destination, release, body, catalogue], {
      cwd: root, env: { ...process.env, PATH: `${bin}:${process.env.PATH}` }, encoding: 'utf8',
    });
    assert.equal(staged.status, 0, staged.stderr);
    const manifest = JSON.parse(await readFile(join(destination, 'workspace.application.json'), 'utf8'));
    for (const name of ['owner-face-component.mjs', 'owner-face-presentation.mjs', 'owner-face-pointer.mjs']) {
      const expected = await readFile(join(root, 'targets/browser/handbook', name));
      assert.deepEqual(await readFile(join(destination, name)), expected);
      const resource = manifest.resources.find(resource => resource.path === name);
      assert(resource);
      assert.equal(resource.sha256, `sha256:${createHash('sha256').update(expected).digest('hex')}`);
      assert(expected.length <= resource.maximum_bytes);
    }
    const template = JSON.parse(await readFile(join(root, 'targets/browser/workspace/workspace.application.template.json'), 'utf8'));
    template.resources = template.resources.filter(resource => !['owner-face-presentation', 'owner-face-pointer'].includes(resource.role));
    template.resources.find(resource => resource.role === 'owner-face-component').dependencies = [];
    const broken = join(scratch, 'broken.json'); await writeFile(broken, JSON.stringify(template));
    const refused = spawnSync(process.execPath, ['targets/browser/tools/build-browser-application-package.mjs',
      broken, destination, 'broken.application.json'], { cwd: root, encoding: 'utf8' });
    assert.notEqual(refused.status, 0);
    assert.match(refused.stderr, /owner-face-component imports undeclared module/);
  } finally { await rm(scratch, { recursive: true, force: true }); }
});
