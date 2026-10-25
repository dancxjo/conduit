// Train evidence admission. This module cannot merge or update a branch.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { verifyBundle } from './receipts.mjs';
import { TARGETS } from './targets.mjs';

const SCHEMA = 'conduit.integration-train/v1';
const sha = value => typeof value === 'string' && /^[a-f0-9]{40}$/.test(value);
const repositoryName = value => typeof value === 'string' && value.length <= 512 && /^[\w.-]+\/[\w.-]+$/.test(value);
const require = (condition, reason) => { if (!condition) throw new Error(reason); };
const digest = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');

function identities(repository, base, inputs) {
  require(repositoryName(repository) && sha(base), 'Invalid train repository or base');
  require(Array.isArray(inputs) && inputs.length > 0 && inputs.length <= 32, 'Train requires 1..32 inputs');
  const seen = new Set();
  const result = inputs.map(input => {
    require(Number.isSafeInteger(input.number) && input.number > 0 && sha(input.head), 'Invalid train input');
    require(!seen.has(input.number), 'Duplicate train pull request');
    seen.add(input.number);
    return { number: input.number, head: input.head };
  }).sort((a, b) => a.number - b.number);
  return { repository, base, inputs: result };
}

function requireReady(identity, pr, run, repository, base) {
  require(pr?.number === identity.number && pr.state === 'open' && pr.draft === false && pr.mergeable === true,
    'Train input is not an open, non-draft, mergeable pull request');
  require(pr.head?.sha === identity.head && pr.head?.repo?.full_name === repository &&
    pr.base?.ref === 'dev' && pr.base?.sha === base && pr.base?.repo?.full_name === repository,
  'Train input head, repository or base changed');
  require(run?.repository?.full_name === repository && run.head_repository?.full_name === repository &&
    run.path === '.github/workflows/candidate.yml' && run.event === 'pull_request' &&
    run.head_sha === identity.head && run.status === 'completed' && run.conclusion === 'success' &&
    run.pull_requests?.some(p => p.number === identity.number),
  'Train input lacks successful exact-head Candidate evidence');
}

export function selectTrainInputs({ repository, base, selections }) {
  require(Array.isArray(selections), 'Missing train selection snapshots');
  const selection = identities(repository, base, selections.map(s => ({ number: s.pr?.number, head: s.pr?.head?.sha })));
  for (const input of selection.inputs) {
    const snapshot = selections.find(s => s.pr.number === input.number);
    requireReady(input, snapshot.pr, snapshot.candidateRun, repository, base);
  }
  return { ...selection, branch: `train/${digest(selection)}` };
}

function payload(manifest) {
  require(manifest?.schema === SCHEMA, 'Unknown train schema');
  const selection = identities(manifest.repository, manifest.base, manifest.inputs);
  require(sha(manifest.source) && sha(manifest.tree), 'Invalid train source or tree');
  const branch = `train/${digest(selection)}`;
  require(manifest.branch === branch, 'Train branch does not bind its selection');
  return { schema: SCHEMA, ...selection, branch, source: manifest.source, tree: manifest.tree };
}

export function verifyTrainManifest(manifest) {
  const value = payload(manifest);
  require(manifest.digest === digest(value), 'Train manifest digest mismatch');
  // Reject fields that could be mistaken for additional authorization or policy.
  require(Object.keys(manifest).sort().join() === [...Object.keys(value), 'digest'].sort().join(),
    'Unknown train manifest field');
  return value;
}

// Snapshots must come from the live repository API. Git verifies actual object
// identity and ancestry; a claimed list of parent hashes is never sufficient.
function selectedSourceTree({ base, inputs, source, directory }) {
  require(sha(source), 'Invalid candidate source');
  const git = args => execFileSync('git', ['-C', directory, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  for (const commit of [base, ...inputs.map(p => p.head), source]) {
    require(git(['rev-parse', '--verify', `${commit}^{commit}`]) === commit, 'Train requires exact commit objects');
    git(['merge-base', '--is-ancestor', commit, source]);
  }
  // Ancestry alone admits arbitrary additional changes. Reconstruct the selected
  // merge tree with Git's default merge strategy, without changing refs/index or
  // the working tree. Synthetic commits retain ancestry for subsequent merges;
  // their identity is deliberately unrelated to the source receipt identity.
  let combined = base;
  let tree = git(['rev-parse', `${base}^{tree}`]);
  for (const input of inputs) {
    tree = git(['merge-tree', '--write-tree', '--no-messages', combined, input.head]);
    require(sha(tree), 'Train selection has an unresolved merge');
    combined = execFileSync('git', ['-C', directory, 'commit-tree', tree,
      '-p', combined, '-p', input.head, '-m', 'Train tree reconstruction'], {
      encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, GIT_AUTHOR_NAME: 'Conduit train', GIT_COMMITTER_NAME: 'Conduit train',
        GIT_AUTHOR_EMAIL: 'train@example.invalid', GIT_COMMITTER_EMAIL: 'train@example.invalid',
        GIT_AUTHOR_DATE: '2000-01-01T00:00:00Z', GIT_COMMITTER_DATE: '2000-01-01T00:00:00Z' },
    }).trim();
  }
  require(git(['rev-parse', `${source}^{tree}`]) === tree,
    'Train source tree differs from the selected PR merge tree');
  return tree;
}

export function verifyCapturedTrainSource(manifest, directory = process.cwd()) {
  const value = verifyTrainManifest(manifest);
  require(selectedSourceTree({ ...value, directory }) === value.tree, 'Captured train tree mismatch');
  return value;
}

export function captureTrain({ repository, base, selections, source, directory = process.cwd() }) {
  const selection = selectTrainInputs({ repository, base, selections });
  const tree = selectedSourceTree({ ...selection, source, directory });
  const value = { schema: SCHEMA, ...selection,
    source, tree };
  return { ...value, digest: digest(value) };
}

// Recheck live inputs immediately before any landing adapter acts. A candidate
// can be valid evidence but no longer eligible to land after dev or a PR moves.
export function verifyTrainRun(manifest, { development, selections, run }) {
  const value = verifyTrainManifest(manifest);
  require(development === value.base, 'Development advanced; train must be rebuilt');
  require(Array.isArray(selections) && selections.length === value.inputs.length, 'Train input set changed');
  const current = identities(value.repository, development,
    selections.map(s => ({ number: s.pr?.number, head: s.pr?.head?.sha })));
  require(JSON.stringify(current.inputs) === JSON.stringify(value.inputs), 'Train input set changed');
  for (const input of value.inputs) {
    const snapshot = selections.find(s => s.pr.number === input.number);
    requireReady(input, snapshot.pr, snapshot.candidateRun, value.repository, value.base);
  }
  requireManualProof(value, run);
  return { ...value, integrationRun: run.id };
}

function requireManualProof(value, run) {
  require(run?.repository?.full_name === value.repository && run.head_repository?.full_name === value.repository &&
    run.path === '.github/workflows/integration.yml' && run.event === 'workflow_dispatch' &&
    run.head_branch === value.branch && run.head_sha === value.source &&
    run.status === 'completed' && run.conclusion === 'success' && Number.isSafeInteger(run.id) && run.id > 0,
  'Train requires successful exhaustive Integration on its exact source branch');
}

// Post-landing publication has a different admission boundary: selected PRs may
// already be merged. Its actual successful dev push must accept the same exact
// source; discovery separately verifies current dev ancestry and authenticates
// the original train manifest from the manual proof's Actions artifact.
export function verifyAcceptedTrainRun(manifest, { run, acceptance }, directory = process.cwd()) {
  const value = verifyCapturedTrainSource(manifest, directory);
  require(acceptance?.repository?.full_name === value.repository &&
    acceptance.head_repository?.full_name === value.repository &&
    acceptance.event === 'push' && acceptance.head_branch === 'dev' &&
    acceptance.path === '.github/workflows/integration.yml' && acceptance.head_sha === value.source &&
    acceptance.status === 'completed' && acceptance.conclusion === 'success' &&
    Number.isSafeInteger(acceptance.id) && acceptance.id > 0,
  'Train publication requires successful exact-source development acceptance');
  requireManualProof(value, run);
  return { ...value, integrationRun: run.id };
}

// Use the existing full file/size/hash/proof-class verifier. A successful run
// metadata record alone does not establish custody of publishable products.
export async function verifyTrainProducts(manifest, observation, directory) {
  const verified = verifyTrainRun(manifest, observation);
  const receipts = await verifyBundle({ directory, sha: verified.source, targets: TARGETS });
  return { ...verified, receipts };
}
