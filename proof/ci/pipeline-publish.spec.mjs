import assert from 'node:assert/strict';
import { copyFileSync, readFileSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { createPublisher } from '../../tools/ci/pipeline/publish.mjs';
import { sealTarget } from '../../tools/ci/pipeline/receipts.mjs';

const source = 'a'.repeat(40);
const previous = 'b'.repeat(40);
const accepted = 'c'.repeat(40);
const tree = 'd'.repeat(40);
const repository = 'example/conduit';
const targets = [{ id: 'hosted-linux', proofClass: 'executable' }];

async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'pipeline-publish-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const directory = path.join(root, 'bundle');
  const product = path.join(directory, 'hosted-linux');
  const storage = path.join(root, 'uploaded');
  await mkdir(product, { recursive: true });
  await mkdir(storage);
  await writeFile(path.join(product, 'conduit'), 'tested binary');
  await sealTarget({ directory: product, target: 'hosted-linux', sha: source, proofClass: 'executable' });
  const state = {
    integration: { repository: { full_name: repository }, head_repository: { full_name: repository },
      event: 'push', head_branch: 'dev', path: '.github/workflows/integration.yml',
      status: 'completed', conclusion: 'success', head_sha: source },
    head: source, main: previous, acceptedTree: tree, ancestor: true,
    dev: '9'.repeat(40), devContainsSource: true,
    open: [], own: [], refs: [], releases: [], assets: [], calls: [], uploads: 0,
  };
  const response = value => ({ status: 0, stdout: typeof value === 'string' ? value : JSON.stringify(value), stderr: '' });
  const command = (program, args, options) => {
    state.calls.push([program, ...args]);
    if (program === 'tar') return spawnSync(program, args, options);
    if (program === 'git') return response(args[1] === 'HEAD' ? state.head : tree);
    assert.equal(program, 'gh');
    if (args[0] === 'repo') return response(repository);
    if (args[0] === 'release') {
      const name = args[1] === 'upload' ? path.basename(args[3]) : args[args.indexOf('--pattern') + 1];
      if (args[1] === 'upload') {
        state.uploads++;
        if (state.failUpload === state.uploads) return { status: 1, stderr: 'simulated upload failure' };
        copyFileSync(args[3], path.join(storage, name));
        state.assets.push({ name, id: state.assets.length + 1 });
        if (state.moveTagDuringUpload) state.tagSha = previous;
      } else {
        copyFileSync(path.join(storage, name), path.join(args[args.indexOf('--dir') + 1], name));
      }
      return response('');
    }
    assert.equal(args[0], 'api');
    const endpoint = args[1].replace(`repos/${repository}/`, '');
    const method = args.includes('--method') ? args[args.indexOf('--method') + 1] : 'GET';
    const fields = Object.fromEntries(args.flatMap((arg, i) => ['-F', '-f'].includes(arg) ? [args[i + 1].split(/=(.*)/s).slice(0, 2)] : []));
    let result;
    if (endpoint === 'actions/runs/123') result = state.integration;
    else if (endpoint === `git/commits/${source}`) result = { tree: { sha: tree } };
    else if (endpoint === `git/commits/${accepted}`) result = state.mainCommit ?? { tree: { sha: state.acceptedTree } };
    else if (endpoint === `git/commits/${previous}`) result = { tree: { sha: tree }, parents: [] };
    else if (endpoint === `git/commits/${'f'.repeat(40)}`) result = { tree: { sha: state.priorSourceTree } };
    else if (endpoint === 'git/ref/heads/dev') result = { object: { sha: state.dev } };
    else if (endpoint === `compare/${source}...${state.dev}`) {
      result = { status: state.devContainsSource ? 'ahead' : 'diverged',
        merge_base_commit: { sha: state.devContainsSource ? source : previous } };
    }
    else if (endpoint === 'git/ref/heads/main') result = { object: { sha: state.main } };
    else if (endpoint.startsWith('compare/')) {
      const ancestor = endpoint.slice('compare/'.length).split('...')[0];
      const allowed = state.ancestor || state.allowedAncestors?.includes(ancestor);
      result = { status: allowed ? 'ahead' : 'diverged', merge_base_commit: { sha: allowed ? ancestor : 'e'.repeat(40) } };
    } else if (endpoint.startsWith('pulls?state=open')) result = state.open;
    else if (endpoint.startsWith('pulls?state=all')) result = state.own;
    else if (endpoint.startsWith('git/matching-refs/heads/')) result = state.refs;
    else if (endpoint.startsWith('git/matching-refs/tags/')) result = state.tagSha ? [{ ref: `refs/tags/release-${source}`, object: { sha: state.tagSha } }] : [];
    else if (endpoint === 'git/refs' && method === 'POST') {
      result = { ref: fields.ref, object: { sha: fields.sha } };
      if (fields.ref.startsWith('refs/tags/')) {
        if (state.failTagCreation) return { status: 1, stderr: 'simulated tag authorization failure' };
        assert.equal(fields.sha, accepted);
        state.tagSha = fields.sha;
      } else state.refs.push(result);
    } else if (endpoint === 'pulls' && method === 'POST') {
      result = { number: 9, state: 'open', head: { sha: source, repo: { full_name: repository }, ref: fields.head } };
      state.own.push(result);
    } else if (endpoint === `statuses/${source}` && method === 'POST') {
      if (state.failStatus) return { status: 1, stderr: 'simulated status failure' };
      assert.equal(fields.state, 'success');
      assert.equal(fields.context, 'release-verification');
      assert.equal(fields.target_url, `https://github.com/${repository}/actions/runs/123`);
      result = fields;
    } else if (endpoint === 'pulls/9/merge' && method === 'PUT') {
      assert.equal(fields.sha, source);
      assert.equal(fields.merge_method, 'merge');
      state.main = accepted;
      state.mainCommit = undefined;
      Object.assign(state.own[0], { state: 'closed', merged_at: '2026-10-02T00:00:00Z', merge_commit_sha: accepted });
      result = { merged: true, sha: accepted };
    } else if (endpoint === 'releases?per_page=100') result = state.releases;
    else if (endpoint === 'releases' && method === 'POST') {
      assert.equal(state.tagSha, accepted, 'exact accepted tag must precede release creation');
      assert.equal(fields.target_commitish, undefined, 'existing tag owns identity without historical target_commitish');
      assert.equal(fields.draft, 'true');
      result = { id: 7, tag_name: fields.tag_name, target_commitish: 'dev', draft: true, prerelease: false };
      state.releases.push(result);
    } else if (endpoint === 'releases/7/assets?per_page=100') result = state.assets;
    else if (endpoint === 'releases/7' && method === 'PATCH') {
      state.releases[0].draft = false;
      state.releases[0].make_latest = fields.make_latest;
      if (fields.make_latest === 'true') state.latestTag = state.releases[0].tag_name;
      result = state.releases[0];
    } else if (endpoint === `commits/release-${source}`) result = { sha: state.tagSha ?? accepted };
    else assert.fail(`Unexpected API operation ${method} ${endpoint}`);
    return response(args.includes('--slurp') ? [result] : result);
  };
  const output = path.join(root, 'output');
  const publish = createPublisher({ command, targets, output });
  return { state, directory, product, output, storage, publish: () => publish('123', directory) };
}

function mutations(state) {
  return state.calls.filter(call => call.includes('POST') || call.includes('PUT') || call.includes('PATCH') || call[2] === 'upload');
}

test('promotes a release PR and publishes verified artifacts with separate source/main identities', async t => {
  const f = await fixture(t);
  const result = await f.publish();
  assert.equal(result.sourceSha, source);
  assert.equal(result.mainSha, accepted);
  assert.equal(result.treeSha, tree);
  assert.equal(f.state.releases[0].draft, false);
  assert.equal(f.state.releases[0].make_latest, 'true');
  assert.equal(f.state.latestTag, `release-${source}`);
  assert.equal(f.state.uploads, 2);
  const statusIndex = f.state.calls.findIndex(call => call[2] === `repos/${repository}/statuses/${source}`);
  const mergeIndex = f.state.calls.findIndex(call => call[2] === `repos/${repository}/pulls/9/merge`);
  assert.ok(statusIndex > 0);
  assert.equal(mergeIndex, statusIndex + 1, 'exact-source status must succeed immediately before guarded merge');
  const manifest = JSON.parse(await readFile(path.join(f.storage, 'manifest.json'), 'utf8'));
  assert.equal(manifest.testedSourceSha, source);
  assert.equal(manifest.acceptedMainSha, accepted);
  assert.equal(manifest.targets[0].proofClass, 'executable');
  assert.equal(await readFile(f.output, 'utf8'), `main-sha=${accepted}\nsource-sha=${source}\n`);
  assert.ok(!f.state.calls.some(call => call.includes('--clobber') || call.includes('cargo')));
  assert.ok(!f.state.calls.some(call => call.includes('PATCH') && call[2].includes('git/refs')));
});

for (const [name, mutate] of [
  ['failed run', s => { s.integration.conclusion = 'failure'; }],
  ['skipped run', s => { s.integration.conclusion = 'skipped'; }],
  ['cancelled run', s => { s.integration.conclusion = 'cancelled'; }],
  ['unfinished run', s => { s.integration.status = 'in_progress'; }],
  ['PR run', s => { s.integration.event = 'pull_request'; }],
  ['wrong branch', s => { s.integration.head_branch = 'main'; }],
  ['wrong workflow', s => { s.integration.path = '.github/workflows/candidate.yml'; }],
  ['wrong repository', s => { s.integration.repository.full_name = 'other/conduit'; }],
  ['fork source', s => { s.integration.head_repository.full_name = 'other/conduit'; }],
  ['wrong checkout', s => { s.head = previous; }],
  ['diverged main', s => { s.ancestor = false; }],
  ['source removed from dev', s => { s.devContainsSource = false; }],
  ['another release', s => { s.open = [{ head: { ref: 'release/another' } }]; }],
  ['open synchronization', s => { s.open = [{ head: { ref: 'sync/another' } }]; }],
]) {
  test(`refuses ${name} before any GitHub mutation`, async t => {
    const f = await fixture(t);
    mutate(f.state);
    await assert.rejects(f.publish());
    assert.equal(mutations(f.state).length, 0);
  });
}

test('tampered bundle cannot promote main', async t => {
  const f = await fixture(t);
  await writeFile(path.join(f.product, 'conduit'), 'tampered');
  await assert.rejects(f.publish(), /digest or file set/);
  assert.equal(mutations(f.state).length, 0);
});

test('a merge tree mismatch refuses release publication', async t => {
  const f = await fixture(t);
  f.state.acceptedTree = 'e'.repeat(40);
  await assert.rejects(f.publish(), /tree differs/);
  assert.equal(f.state.uploads, 0);
  assert.equal(f.state.releases.length, 0);
});

test('resumes a partially uploaded draft without merging or replacing verified assets', async t => {
  const f = await fixture(t);
  f.state.failUpload = 2;
  await assert.rejects(f.publish(), /upload failure/);
  assert.equal(f.state.assets.length, 1);
  assert.equal(f.state.releases[0].draft, true);
  const mergeCount = () => f.state.calls.filter(call => call[2]?.endsWith('/merge')).length;
  assert.equal(mergeCount(), 1);
  await f.publish();
  assert.equal(mergeCount(), 1);
  assert.equal(f.state.assets.length, 2);
  assert.equal(f.state.uploads, 3);
  assert.equal(f.state.releases[0].draft, false);
  // A fully completed rerun only checks existing assets and identities.
  await f.publish();
  assert.equal(mergeCount(), 1);
  assert.equal(f.state.uploads, 3);
});

test('an existing different asset is never overwritten', async t => {
  const f = await fixture(t);
  await f.publish();
  await writeFile(path.join(f.storage, 'hosted-linux.tar.gz'), 'different bytes');
  await assert.rejects(f.publish(), /Existing asset differs/);
  assert.equal(f.state.uploads, 2);
  assert.equal(readFileSync(path.join(f.storage, 'hosted-linux.tar.gz'), 'utf8'), 'different bytes');
});

test('a moved release tag is refused on rerun', async t => {
  const f = await fixture(t);
  await f.publish();
  f.state.tagSha = previous;
  await assert.rejects(f.publish(), /tag has moved/);
  assert.equal(f.state.uploads, 2);
});


test('a second release accepts the prior source lineage without syncing the main merge to dev', async t => {
  const f = await fixture(t);
  const priorSource = 'f'.repeat(40);
  f.state.main = accepted;
  f.state.ancestor = false;
  f.state.allowedAncestors = [priorSource];
  f.state.priorSourceTree = 'e'.repeat(40);
  f.state.mainCommit = { tree: { sha: f.state.priorSourceTree }, parents: [{ sha: previous }, { sha: priorSource }] };
  const result = await f.publish();
  assert.equal(result.treeSha, tree);
  assert.equal(f.state.uploads, 2);
});

test('main-only content in a previous release merge blocks the next promotion', async t => {
  const f = await fixture(t);
  f.state.main = accepted;
  f.state.ancestor = false;
  f.state.priorSourceTree = 'e'.repeat(40);
  f.state.mainCommit = { tree: { sha: tree }, parents: [{ sha: previous }, { sha: 'f'.repeat(40) }] };
  await assert.rejects(f.publish(), /tree differs/);
  assert.equal(mutations(f.state).length, 0);
});


test('failed commit status prevents the release PR merge and publication', async t => {
  const f = await fixture(t);
  f.state.failStatus = true;
  await assert.rejects(f.publish(), /status failure/);
  assert.equal(f.state.main, previous);
  assert.ok(!f.state.calls.some(call => call[2]?.endsWith('/merge')));
  assert.equal(f.state.uploads, 0);
  assert.equal(f.state.releases.length, 0);
});


test('tag authorization failure leaves main accepted but creates no release or assets', async t => {
  const f = await fixture(t);
  f.state.failTagCreation = true;
  await assert.rejects(f.publish(), /tag authorization failure/);
  assert.equal(f.state.main, accepted);
  assert.equal(f.state.releases.length, 0);
  assert.equal(f.state.uploads, 0);
});

test('pre-existing wrong tag is never replaced', async t => {
  const f = await fixture(t);
  f.state.tagSha = previous;
  await assert.rejects(f.publish(), /tag has moved/);
  assert.equal(f.state.tagSha, previous);
  assert.equal(f.state.releases.length, 0);
});

test('tag movement while assets upload prevents publishing the draft', async t => {
  const f = await fixture(t);
  f.state.moveTagDuringUpload = true;
  await assert.rejects(f.publish(), /tag changed before publication/);
  assert.equal(f.state.releases[0].draft, true);
});


test('resuming a historical draft preserves the newer latest release', async t => {
  const f = await fixture(t);
  f.state.failUpload = 2;
  await assert.rejects(f.publish(), /upload failure/);
  assert.equal(f.state.releases[0].draft, true);
  // Another tested batch has since advanced main and published successfully.
  f.state.main = '8'.repeat(40);
  f.state.latestTag = 'release-newer-tested-source';
  await f.publish();
  assert.equal(f.state.releases[0].draft, false);
  assert.equal(f.state.releases[0].make_latest, 'false');
  assert.equal(f.state.latestTag, 'release-newer-tested-source');
  assert.equal(f.state.assets.length, 2);
});
