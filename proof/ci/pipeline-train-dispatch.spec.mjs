import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { commands } from './helpers/train-command-fixture.mjs';
import { createTrainDispatcher } from '../../tools/ci/pipeline/train-dispatch.mjs';

async function fixture(t) {
  const f = await commands(t);
  await f.api.capture('example/conduit', f.source, [1, 2], f.filename);
  const state = { ref: null, runs: [], writes: [], moveAfterCreation: false, failDispatch: false };
  const fullRef = `refs/heads/${f.manifest.branch}`;
  const command = (program, args, options) => {
    if (program !== 'gh') return f.command(program, args, options);
    const endpoint = args[1];
    if (args.includes('--method')) {
      assert.equal(args[args.indexOf('--method') + 1], 'POST');
      assert.equal(args.at(-1), '-');
      const body = JSON.parse(options.input); state.writes.push({ endpoint, body });
      if (endpoint.endsWith('/git/refs')) {
        assert.deepEqual(body, { ref: fullRef, sha: f.source });
        assert.equal(state.ref, null);
        state.ref = { ref: fullRef, object: { sha: f.source } };
        return { status: 0, stdout: JSON.stringify(state.ref) };
      }
      assert.ok(endpoint.endsWith('/integration.yml/dispatches'));
      assert.deepEqual(body, { ref: f.manifest.branch, inputs: { 'train-source': f.source } });
      return state.failDispatch ? { status: 1, stderr: 'transport outcome unknown' } : { status: 0, stdout: '' };
    }
    if (endpoint.includes('/git/matching-refs/')) return { status: 0, stdout: JSON.stringify(state.ref ? [state.ref] : []) };
    if (endpoint.includes('/integration.yml/runs?')) return { status: 0, stdout: JSON.stringify({ workflow_runs: state.runs }) };
    if (endpoint.includes('/git/ref/heads/train/')) return { status: 0,
      stdout: JSON.stringify(state.moveAfterCreation ? { object: { sha: f.base } } : state.ref) };
    return f.command(program, args, options);
  };
  return { ...f, state, fullRef, dispatch: createTrainDispatcher({ command, directory: f.directory }) };
}

test('dispatch creates the exact train ref and passes the immutable source guard', async t => {
  const f = await fixture(t);
  const result = await f.dispatch(f.filename);
  assert.equal(result.dispatched, true); assert.equal(result.runId, null);
  assert.equal(result.manifestDigest, f.manifest.digest);
  assert.equal(f.state.writes.length, 2);
  // No immediately-visible run is not permission to send a duplicate request.
  await assert.rejects(f.dispatch(f.filename), /requires inspection/);
  assert.equal(f.state.writes.length, 2);
});

test('known queued, running and successful exact-source runs are reused without mutations', async t => {
  const f = await fixture(t);
  f.state.ref = { ref: f.fullRef, object: { sha: f.source } };
  for (const status of ['queued', 'in_progress', 'completed']) {
    f.state.runs = [{ ...f.observation.run, status, conclusion: status === 'completed' ? 'success' : null }];
    const result = await f.dispatch(f.filename);
    assert.equal(result.runId, 123); assert.equal(result.reused, true);
  }
  assert.equal(f.state.writes.length, 0);
});

test('newer failed proof and moved train refs refuse without restarting CI', async t => {
  const f = await fixture(t);
  f.state.ref = { ref: f.fullRef, object: { sha: f.source } };
  f.state.runs = [{ ...f.observation.run }, { ...f.observation.run, id: 124, conclusion: 'failure' }];
  await assert.rejects(f.dispatch(f.filename), /proof failed/);
  f.state.ref.object.sha = f.base;
  await assert.rejects(f.dispatch(f.filename), /differs from source/);
  assert.equal(f.state.writes.length, 0);
});

test('failed input admission cannot create refs or dispatch workflows', async t => {
  const f = await fixture(t);
  f.runs[0][0].conclusion = 'failure';
  await assert.rejects(f.dispatch(f.filename), /Candidate/);
  assert.equal(f.state.writes.length, 0);
});

test('a ref race after creation stops before expensive workflow dispatch', async t => {
  const f = await fixture(t);
  f.state.moveAfterCreation = true;
  await assert.rejects(f.dispatch(f.filename), /moved before dispatch/);
  assert.equal(f.state.writes.length, 1);
});

test('ambiguous dispatch transport failure preserves intent and refuses automatic retry', async t => {
  const f = await fixture(t);
  f.state.failDispatch = true;
  await assert.rejects(f.dispatch(f.filename), /outcome unknown/);
  await assert.rejects(f.dispatch(f.filename), /requires inspection/);
  assert.equal(f.state.writes.length, 2);
});

test('Integration guards manual train source before proof while retaining normal dev proof', async () => {
  const workflow = await readFile('.github/workflows/integration.yml', 'utf8');
  assert.match(workflow, /train-source:[\s\S]*EXPECTED_SOURCE: \$\{\{ inputs\.train-source \}\}/);
  assert.match(workflow, /ACTUAL_SOURCE: \$\{\{ github\.sha \}\}/);
  assert.match(workflow, /run: test "\$EXPECTED_SOURCE" = "\$ACTUAL_SOURCE"/);
  assert.match(workflow, /proof:\n\s+needs: train-source/);
  assert.match(workflow, /needs\.train-source\.result == 'skipped'/);
  assert.match(workflow, /base: all/);
  assert.match(workflow, /cancel-in-progress: false/);
});
