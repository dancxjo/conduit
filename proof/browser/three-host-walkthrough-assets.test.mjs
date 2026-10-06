import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { verifyWalkthroughAssets } from './three-host-walkthrough-assets.mjs';

test('checks local media while allowing navigation and in-page links', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'conduit-walkthrough-assets-'));
  try {
    await mkdir(path.join(root, 'speech'));
    await writeFile(path.join(root, 'speech', 'play.wav'), 'observed');
    await verifyWalkthroughAssets(root,
      '<a href="#birth">Birth</a><a href="https://dancxjo.github.io/conduit/">Home</a>' +
      '<audio src="speech/play.wav"></audio>');
    await assert.rejects(verifyWalkthroughAssets(root, '<img src="missing.png">'),
      /missing or unsafe/);
    await assert.rejects(verifyWalkthroughAssets(root, '<a href="../private.json">Private</a>'),
      /escapes its bundle/);
    await symlink(path.join(root, 'speech', 'play.wav'), path.join(root, 'linked.wav'));
    await assert.rejects(verifyWalkthroughAssets(root, '<audio src="linked.wav"></audio>'),
      /missing or unsafe/);
  } finally { await rm(root, { recursive: true, force: true }); }
});
