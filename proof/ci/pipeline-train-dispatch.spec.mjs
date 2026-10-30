import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { readFile, writeFile, stat } from 'node:fs/promises';
import test from 'node:test';
import { commands } from './helpers/train-command-fixture.mjs';
import { createTrainDispatcher } from '../../tools/ci/pipeline/train-dispatch.mjs';

async function fixture(t) {
  const f = await commands(t);
  await f.api.capture('example/conduit', f.source, [1, 2], f.filename);
  const server = path.join(f.output, 'server.git');
  const serverGit = args => spawnSync('git', ['--git-dir', server, ...args], { encoding: 'utf8' });
  assert.equal(spawnSync('git', ['init', '--bare', server], { encoding: 'utf8' }).status, 0);
  const state = { ref: null, runs: [], writes: [], moveAfterCreation: false, failDispatch: false,
    raceSource: null, failPush: false, ambiguousAck: false, staleAfterInventory: false };
  assert.notEqual(serverGit(['cat-file', '-e', f.source]).status, 0);
  const fullRef = `refs/heads/${f.manifest.branch}`;
  const command = (program, args, options) => {
    if (program === 'git' && args.includes('push')) {
      const remote = `https://github.com/${f.manifest.repository}.git`;
      assert.deepEqual(args, ['-c', 'credential.helper=', '-c', 'credential.helper=!gh auth git-credential',
        '-c', 'credential.interactive=false', 'push', '--porcelain', '--no-follow-tags', '--recurse-submodules=no', `--force-with-lease=${fullRef}:`,
        remote, `${f.source}:${fullRef}`]);
      assert.equal(options.env.GIT_TERMINAL_PROMPT, '0');
      assert.notEqual(options.cwd, f.directory);
      assert.equal(options.env.GIT_CONFIG_GLOBAL, '/dev/null');
      assert.equal(options.env.GIT_CONFIG_NOSYSTEM, '1');
      if (state.raceSource) {
        const raced = spawnSync('git', ['push', server, `${state.raceSource}:${fullRef}`], { cwd: f.directory, encoding: 'utf8' });
        assert.equal(raced.status, 0, raced.stderr);
      }
      state.writes.push({ push: true });
      // Production retains its canonical HTTPS destination. Only this test's
      // process boundary substitutes an isolated, initially empty bare server.
      const result = spawnSync(program, args.map(arg => arg === remote ? server : arg), options);
      const ref = serverGit(['rev-parse', '--verify', fullRef]);
      if (ref.status === 0) state.ref = { ref: fullRef, object: { sha: ref.stdout.trim() } };
      if (state.failPush) return { status: 1, stderr: 'secret credential in transport diagnostic' };
      if (state.ambiguousAck) return { ...result, stdout: '' };
      return result;
    }
    if (program !== 'gh') return f.command(program, args, options);
    const endpoint = args[1];
    if (args.includes('--method')) {
      assert.equal(args[args.indexOf('--method') + 1], 'POST');
      assert.equal(args.at(-1), '-');
      const body = JSON.parse(options.input); state.writes.push({ endpoint, body });
      assert.ok(endpoint.endsWith('/integration.yml/dispatches'));
      assert.deepEqual(body, { ref: f.manifest.branch,
        inputs: { 'train-source': f.source, 'train-manifest': JSON.stringify(f.manifest) } });
      return state.failDispatch ? { status: 1, stderr: 'transport outcome unknown' } : { status: 0, stdout: '' };
    }
    if (endpoint.includes('/git/matching-refs/')) return { status: 0, stdout: JSON.stringify(state.ref ? [state.ref] : []) };
    if (endpoint.includes('/integration.yml/runs?')) {
      if (state.staleAfterInventory) f.runs[0][0].conclusion = 'failure';
      return { status: 0, stdout: JSON.stringify({ workflow_runs: state.runs }) };
    }
    if (endpoint.includes('/git/ref/heads/train/')) return { status: 0,
      stdout: JSON.stringify(state.moveAfterCreation ? { object: { sha: f.base } } : state.ref) };
    return f.command(program, args, options);
  };
  return { ...f, state, fullRef, serverGit, dispatch: createTrainDispatcher({ command, directory: f.directory }) };
}

test('dispatch creates the exact train ref and passes the immutable source guard', async t => {
  const f = await fixture(t);
  f.git('tag', '-a', 'retained-local-tag', '-m', 'Local tag must not accompany train publication');
  f.git('config', 'push.followTags', 'true');
  const hookMarker = path.join(f.output, 'source-hook-ran');
  await writeFile(path.join(f.directory, '.git/hooks/pre-push'),
    `#!/bin/sh\necho observed > '${hookMarker}'\n`, { mode: 0o700 });
  const result = await f.dispatch(f.filename);
  assert.notEqual(f.serverGit(['rev-parse', '--verify', 'refs/tags/retained-local-tag']).status, 0);
  assert.equal(result.dispatched, true); assert.equal(result.runId, null);
  await assert.rejects(stat(hookMarker), { code: 'ENOENT' });
  assert.equal(result.manifestDigest, f.manifest.digest);
  assert.equal(f.serverGit(['cat-file', '-t', f.source]).stdout.trim(), 'commit');
  assert.equal(f.serverGit(['rev-parse', f.fullRef]).stdout.trim(), f.source);
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

test('readiness becoming stale after inventory prevents object transfer and ref creation', async t => {
  const f = await fixture(t); f.state.staleAfterInventory = true;
  await assert.rejects(f.dispatch(f.filename), /Candidate/);
  assert.equal(f.state.writes.length, 0);
  assert.notEqual(f.serverGit(['cat-file', '-e', f.source]).status, 0);
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


test('empty expected-ref lease refuses a competing different or identical source without duplicate dispatch', async t => {
  for (const identical of [false, true]) {
    const f = await fixture(t);
    f.state.raceSource = identical ? f.source : f.base;
    await assert.rejects(f.dispatch(f.filename), identical ? /not acknowledged/ : /push failed/);
    assert.equal(f.state.writes.length, 1);
    assert.equal(f.state.ref.object.sha, f.state.raceSource);
    await assert.rejects(f.dispatch(f.filename), identical ? /requires inspection/ : /differs from source/);
    assert.equal(f.state.writes.length, 1);
  }
});

test('existing identical ref without proof never grants dispatch ownership', async t => {
  const f = await fixture(t);
  f.state.ref = { ref: f.fullRef, object: { sha: f.source } };
  await assert.rejects(f.dispatch(f.filename), /requires inspection/);
  assert.equal(f.state.writes.length, 0);
});

for (const mode of ['failPush', 'ambiguousAck']) {
  test(`${mode} after actual object transfer preserves intent and refuses dispatch or retry`, async t => {
    const f = await fixture(t); f.state[mode] = true;
    await assert.rejects(f.dispatch(f.filename), error => {
      assert.doesNotMatch(error.message, /secret credential/);
      return /transport outcome unknown|not acknowledged/.test(error.message);
    });
    assert.equal(f.serverGit(['cat-file', '-t', f.source]).stdout.trim(), 'commit');
    assert.equal(f.state.ref.object.sha, f.source);
    assert.equal(f.state.writes.length, 1);
    await assert.rejects(f.dispatch(f.filename), /requires inspection/);
    assert.equal(f.state.writes.length, 1);
  });
}

test('Integration guards manual train source before proof while retaining normal dev proof', async () => {
  const workflow = await readFile('.github/workflows/integration.yml', 'utf8');
  assert.match(workflow, /train-source:[\s\S]*EXPECTED_SOURCE: \$\{\{ inputs\.train-source \}\}/);
  assert.match(workflow, /ACTUAL_SOURCE: \$\{\{ github\.sha \}\}/);
  assert.match(workflow, /run: test "\$EXPECTED_SOURCE" = "\$ACTUAL_SOURCE"/);
  assert.match(workflow, /proof:\n\s+permissions:\n\s+contents: read\n\s+actions: read\n\s+needs: \[train-source, adoption\]/);
  assert.match(workflow, /needs\.train-source\.result == 'skipped'/);
  assert.match(workflow, /base: all/);
  assert.match(workflow, /cancel-in-progress: false/);
});


test('only Integration earns train-accepted after exhaustive proof or authenticated adoption', async () => {
  const workflow = await readFile('.github/workflows/integration.yml', 'utf8');
  const candidate = await readFile('.github/workflows/candidate.yml', 'utf8');
  assert.match(workflow, /\n  train-accepted:\n/);
  assert.doesNotMatch(workflow, /\n  candidate:\n/);
  assert.doesNotMatch(candidate, /train-accepted/);
  const aggregate = workflow.split('  train-accepted:\n')[1];
  assert.match(aggregate, /if: \$\{\{ always\(\) \}\}/);
  assert.match(aggregate, /needs: \[train-source, adoption, proof\]/);
  const script = aggregate.split('        run: |\n')[1].split('\n      - ')[0].split('\n').map(line => line.slice(10)).join('\n');
  for (const [guard, adoption, adopted, result, accepted] of [
    ['success', 'skipped', '', 'success', true],
    ['skipped', 'success', 'false', 'success', true],
    ['skipped', 'success', 'true', 'skipped', true],
    ['failure', 'skipped', '', 'success', false],
    ['cancelled', 'skipped', '', 'success', false],
    ['success', 'skipped', '', 'skipped', false],
    ['success', 'skipped', '', 'failure', false],
    ['skipped', 'failure', 'true', 'skipped', false],
    ['skipped', 'success', 'true', 'success', false],
    ['skipped', 'success', '', 'success', false],
  ]) {
    const outcome = spawnSync('bash', ['-e', '-c', script], { env: { ...process.env,
      GUARD: guard, ADOPTION: adoption, ADOPTED: adopted, RESULT: result } });
    assert.equal(outcome.status === 0, accepted, JSON.stringify({ guard, adoption, adopted, result }));
  }
});


test('legacy default dispatch refuses absent explicit token before reading source or stored authentication', () => {
  const environment = { ...process.env }; delete environment.GH_TOKEN; delete environment.GITHUB_TOKEN;
  const script = `import {createTrainDispatcher} from './tools/ci/pipeline/train-dispatch.mjs';
try { await createTrainDispatcher()('/missing-fixture-manifest'); process.exit(2); }
catch(error) { process.exit(error.message.includes('scoped installation credential required') ? 0 : 3); }`;
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', script], { env: environment, encoding: 'utf8' });
  assert.equal(result.status, 0);
});
