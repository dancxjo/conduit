import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { machinePolicyFixture } from './helpers/train-machine-policy-fixture.mjs';

const controller = fileURLToPath(new URL('../../', import.meta.url));
const cli = path.join(controller, 'tools/ci/pipeline/cli.mjs');

test('landing entrance separates trusted tooling cwd from explicit target data cwd', async t => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'train-land-cli-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const target = path.join(root, 'proposed'); await mkdir(target);
  const config = path.join(root, 'policy.json');
  await writeFile(config, JSON.stringify(machinePolicyFixture().config));
  const args = ['train-land', path.join(root, 'missing-train.json'), '1',
    path.join(root, 'archives'), path.join(root, 'new-evidence'), config,
    'CONDUIT_TEST_APP_JWT', 'CONDUIT_TEST_POLICY_OBSERVER', target];
  const invoke = argv => spawnSync(process.execPath, [cli, ...argv], {
    cwd: controller, encoding: 'utf8', timeout: 10000,
    env: { ...process.env, CONDUIT_TEST_APP_JWT: 'fixture-only-jwt',
      CONDUIT_TEST_POLICY_OBSERVER: 'fixture-only-observer' },
  });

  // The old implicit target location is no longer an accepted interface.
  const implicit = invoke(args.slice(0, -1));
  assert.equal(implicit.status, 1);
  assert.ok(implicit.stderr.includes('<target-source-directory>'));

  // Separate proposed data proceeds to manifest admission without executing
  // proposed tooling or reaching a network/credential observation.
  const separate = invoke(args);
  assert.equal(separate.status, 1);
  assert.ok(separate.stderr.includes('ENOENT') && separate.stderr.includes('missing-train.json'));

  const same = invoke([...args.slice(0, -1), controller]);
  assert.equal(same.status, 1);
  assert.ok(same.stderr.includes('separately trusted controller'));

  // The public policy file must remain external to the proposed source too.
  const inside = path.join(target, 'policy.json');
  await writeFile(inside, JSON.stringify(machinePolicyFixture().config));
  const insideArgs = [...args]; insideArgs[5] = inside;
  const contained = invoke(insideArgs);
  assert.equal(contained.status, 1);
  assert.ok(contained.stderr.includes('external canonical configuration'));
});
