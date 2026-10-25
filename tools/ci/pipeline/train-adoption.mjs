// Discover authenticated manual train proof for an already accepted dev source.
// Discovery authorizes neither artifact substitution nor publication on its own.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { verifyTrainManifest, verifyCapturedTrainSource } from './train.mjs';

const fail = reason => { throw new Error(reason); };
const positive = value => Number.isSafeInteger(value) && value > 0;
const sha = value => typeof value === 'string' && /^[a-f0-9]{40}$/.test(value);
const ZIP_LIMIT = 256 * 1024;
const MANIFEST_LIMIT = 65536;
const decodeZip = `import io,stat,sys,zipfile
raw=sys.stdin.buffer.read(262145)
if len(raw)>262144: raise ValueError('Manifest ZIP exceeds bound')
with zipfile.ZipFile(io.BytesIO(raw)) as z:
 entries=z.infolist()
 if len(entries)!=1: raise ValueError('Manifest ZIP requires one member')
 e=entries[0]; mode=e.external_attr>>16
 if e.filename!='train-manifest.json' or e.is_dir() or (mode and stat.S_IFMT(mode) not in (0,stat.S_IFREG)): raise ValueError('Unsafe manifest member')
 if e.flag_bits&1 or e.compress_type not in (zipfile.ZIP_STORED,zipfile.ZIP_DEFLATED) or not 0<e.file_size<=65536: raise ValueError('Invalid manifest expansion')
 with z.open(e) as f:
  data=f.read(65537)
  if len(data)!=e.file_size or len(data)>65536 or f.read(1): raise ValueError('Manifest expansion differs')
 sys.stdout.buffer.write(data)
`;

export async function discoverTrainAdoption({ pushRun, repository, directory = process.cwd(), command = spawnSync }) {
  if (typeof repository !== 'string' || !/^[\w.-]+\/[\w.-]+$/.test(repository)
      || !positive(pushRun?.id) || !positive(pushRun.run_attempt) || !sha(pushRun.head_sha)) fail('Invalid dev push observation');
  const base = `repos/${repository}`;
  const invoke = (program, args, options = {}) => {
    const result = command(program, args, { cwd: directory, timeout: 60000, maxBuffer: 2 * 1024 ** 2,
      encoding: 'utf8', ...options });
    if (result.error || result.status !== 0) fail(`Train adoption observation failed: ${program}`);
    return result.stdout;
  };
  const api = endpoint => JSON.parse(invoke('gh', ['api', endpoint]));
  const source = pushRun.head_sha;
  const validatePush = run => {
    if (run.id !== pushRun.id || run.run_attempt !== pushRun.run_attempt || run.head_sha !== source
        || run.repository?.full_name !== repository || run.head_repository?.full_name !== repository
        || !positive(run.repository?.id) || run.head_repository?.id !== run.repository.id
        || run.event !== 'push' || run.head_branch !== 'dev' || run.path !== '.github/workflows/integration.yml'
        || !['queued', 'in_progress', 'requested', 'waiting', 'pending', 'completed'].includes(run.status)
        || (run.status === 'completed' && run.conclusion !== 'success')) fail('Actual dev push run differs');
    return run;
  };
  const actualPush = validatePush(api(`${base}/actions/runs/${pushRun.id}`));
  const accepted = () => {
    const development = api(`${base}/git/ref/heads/dev`).object?.sha;
    if (!sha(development)) fail('Invalid accepted development identity');
    const comparison = api(`${base}/compare/${source}...${development}`);
    if (!['ahead', 'identical'].includes(comparison.status) || comparison.merge_base_commit?.sha !== source) fail('Train source is not accepted in development');
    return development;
  };
  accepted();
  const observeLatest = () => {
    const inventory = api(`${base}/actions/workflows/integration.yml/runs?head_sha=${source}&event=workflow_dispatch&per_page=100`);
    if (!Number.isSafeInteger(inventory.total_count) || inventory.total_count < 0 || inventory.total_count > 100
        || !Array.isArray(inventory.workflow_runs) || inventory.workflow_runs.length !== inventory.total_count) fail('Incomplete manual train inventory');
    const candidates = inventory.workflow_runs.filter(run => run.head_sha === source && run.head_branch?.startsWith('train/'));
    if (candidates.some(run => !positive(run.id) || !/^train\/[a-f0-9]{64}$/.test(run.head_branch))) fail('Malformed manual train run');
    return candidates.sort((a, b) => b.id - a.id)[0] ?? null;
  };
  const selected = observeLatest();
  if (!selected) return null;
  const validateManual = run => {
    if (run.id !== selected.id || !positive(run.run_attempt) || !positive(run.check_suite_id)
        || run.event !== 'workflow_dispatch' || run.path !== '.github/workflows/integration.yml'
        || run.head_sha !== source || run.head_branch !== selected.head_branch
        || run.repository?.full_name !== repository || run.head_repository?.full_name !== repository
        || run.repository?.id !== actualPush.repository.id || run.head_repository?.id !== actualPush.repository.id
        || run.status !== 'completed' || run.conclusion !== 'success') fail('Latest exact train proof is not successful');
    return run;
  };
  const run = validateManual(api(`${base}/actions/runs/${selected.id}`));
  if (run.run_attempt !== selected.run_attempt || selected.status !== 'completed' || selected.conclusion !== 'success') fail('Manual train observation changed');
  const checks = api(`${base}/check-suites/${run.check_suite_id}/check-runs?check_name=candidate&per_page=100`);
  const check = checks.check_runs?.[0];
  if (checks.total_count !== 1 || checks.check_runs?.length !== 1 || check.name !== 'candidate'
      || check.app?.id !== 15368 || check.check_suite?.id !== run.check_suite_id
      || check.head_sha !== source || check.status !== 'completed' || check.conclusion !== 'success') fail('Manual train lacks its exact successful Candidate aggregate');
  const inventory = api(`${base}/actions/runs/${run.id}/artifacts?per_page=100`);
  if (!Number.isSafeInteger(inventory.total_count) || inventory.total_count < 0 || inventory.total_count > 100
      || !Array.isArray(inventory.artifacts) || inventory.artifacts.length !== inventory.total_count) fail('Incomplete train manifest inventory');
  const manifests = inventory.artifacts.filter(artifact => artifact.name === 'train-manifest');
  if (manifests.length !== 1) fail('Missing or duplicate train manifest artifact');
  const artifact = api(`${base}/actions/artifacts/${manifests[0].id}`);
  const owner = artifact.workflow_run;
  if (!positive(artifact.id) || artifact.id !== manifests[0].id || artifact.name !== 'train-manifest'
      || artifact.expired !== false || !positive(artifact.size_in_bytes) || artifact.size_in_bytes > ZIP_LIMIT
      || !/^sha256:[a-f0-9]{64}$/.test(artifact.digest ?? '')
      || owner?.id !== run.id || owner.head_sha !== source
      || owner.repository_id !== actualPush.repository.id || owner.head_repository_id !== actualPush.repository.id) fail('Invalid run-bound manifest artifact');
  const archive = invoke('gh', ['api', `${base}/actions/artifacts/${artifact.id}/zip`], { encoding: null, maxBuffer: ZIP_LIMIT });
  if (!Buffer.isBuffer(archive) || archive.length !== artifact.size_in_bytes
      || `sha256:${createHash('sha256').update(archive).digest('hex')}` !== artifact.digest) fail('Manifest archive digest or size mismatch');
  const json = invoke('python3', ['-c', decodeZip], { input: archive, maxBuffer: MANIFEST_LIMIT });
  const manifest = JSON.parse(json);
  verifyTrainManifest(manifest);
  if (manifest.repository !== repository || manifest.source !== source || manifest.branch !== run.head_branch) fail('Authenticated manifest differs from manual proof');
  verifyCapturedTrainSource(manifest, directory);
  if (api(`${base}/git/commits/${source}`).tree?.sha !== manifest.tree) fail('Accepted source tree differs from train manifest');
  const latest = observeLatest();
  const current = validateManual(api(`${base}/actions/runs/${run.id}`));
  if (!latest || latest.id !== run.id || latest.run_attempt !== run.run_attempt
      || latest.status !== 'completed' || latest.conclusion !== 'success'
      || current.run_attempt !== run.run_attempt) fail('Train proof attempt changed during adoption');
  validatePush(api(`${base}/actions/runs/${pushRun.id}`));
  return { manifest, run, acceptedSource: source, development: accepted(),
    integrationRun: run.id, runAttempt: run.run_attempt, manifestDigest: manifest.digest,
    manifestArtifact: { id: artifact.id, digest: artifact.digest, bytes: artifact.size_in_bytes } };
}
