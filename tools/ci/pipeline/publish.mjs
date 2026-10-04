import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { appendFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { TARGETS } from './targets.mjs';
import { verifyBundle } from './receipts.mjs';

const SHA = /^[a-f0-9]{40}$/;
const fail = message => { throw new Error(message); };

export function validateIntegrationRun(run, repository, checkout) {
  if (run.repository?.full_name !== repository || run.head_repository?.full_name !== repository ||
      run.event !== 'push' || run.head_branch !== 'dev' ||
      run.path !== '.github/workflows/integration.yml' ||
      run.status !== 'completed' || run.conclusion !== 'success' ||
      !SHA.test(run.head_sha ?? '') || run.head_sha !== checkout) {
    fail('Publication requires a successful dev integration run from this repository at checkout HEAD');
  }
  return run.head_sha;
}

export function requireAncestor(comparison, ancestor) {
  if (!['ahead', 'identical'].includes(comparison.status) || comparison.merge_base_commit?.sha !== ancestor) {
    fail('Accepted main must be an ancestor of the tested source');
  }
}

export function requireTreeIdentity(sourceTree, acceptedTree) {
  if (!SHA.test(sourceTree ?? '') || sourceTree !== acceptedTree) {
    fail('Accepted main tree differs from the tested source tree; publication refused');
  }
}

function isAncestor(comparison, ancestor) {
  return ['ahead', 'identical'].includes(comparison.status) && comparison.merge_base_commit?.sha === ancestor;
}

function releaseLane(head) {
  return /^(release\/|sync\/|release-sync\/|sync-release\/)/.test(head ?? '');
}

async function digest(filename) {
  const hash = createHash('sha256');
  for await (const bytes of createReadStream(filename)) hash.update(bytes);
  return hash.digest('hex');
}

// Injection keeps refusal and recovery tests offline. Production always uses gh
// with the publication job's token, and performs no test or build commands.
export function createPublisher({ command = spawnSync, targets = TARGETS, output = process.env.GITHUB_OUTPUT,
  repositoryName = process.env.GITHUB_REPOSITORY } = {}) {
  const run = (program, args) => {
    const result = command(program, args, { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
    if (result.error || result.status !== 0) {
      fail(`${program} ${args[0]} failed: ${result.error?.message ?? result.stderr ?? result.status}`);
    }
    return result.stdout.trim();
  };
  const api = (endpoint, method = 'GET', fields = {}) => {
    const args = ['api', endpoint, '--method', method];
    for (const [key, value] of Object.entries(fields)) args.push(typeof value === 'boolean' ? '-F' : '-f', `${key}=${value}`);
    return JSON.parse(run('gh', args));
  };
  const pages = endpoint => JSON.parse(run('gh', ['api', endpoint, '--paginate', '--slurp'])).flat();

  return async function publish(runId, directory) {
    if (!/^[1-9][0-9]*$/.test(String(runId))) fail('A numeric integration run ID is required');
    // Actions supplies this identity directly. Avoid an extra GraphQL request
    // before the REST integration-run check, while keeping local invocation.
    const repository = repositoryName || run('gh', ['repo', 'view', '--json', 'nameWithOwner', '--jq', '.nameWithOwner']);
    if (!/^[\w.-]+\/[\w.-]+$/.test(repository)) fail('Invalid repository identity');
    const base = `repos/${repository}`;
    const checkout = run('git', ['rev-parse', 'HEAD']);
    const integration = api(`${base}/actions/runs/${runId}`);
    const source = validateIntegrationRun(integration, repository, checkout);
    const development = api(`${base}/git/ref/heads/dev`).object?.sha;
    if (!SHA.test(development ?? '')) fail('Invalid development branch identity');
    requireAncestor(api(`${base}/compare/${source}...${development}`), source);
    const receipts = await verifyBundle({ directory, sha: source, targets });
    const sourceTree = api(`${base}/git/commits/${source}`).tree?.sha;
    requireTreeIdentity(sourceTree, run('git', ['rev-parse', `${source}^{tree}`]));
    const branch = `release/${source}`;
    const tag = `release-${source}`;
    const open = pages(`${base}/pulls?state=open&per_page=100`);
    if (open.some(pr => releaseLane(pr.head?.ref) && (pr.head?.ref !== branch || pr.head?.repo?.full_name !== repository))) {
      fail('Another release or synchronization PR is open');
    }
    const own = pages(`${base}/pulls?state=all&base=main&head=${repository.split('/')[0]}:${branch}&per_page=100`);
    if (own.some(pr => pr.head?.sha !== source || pr.head?.repo?.full_name !== repository)) {
      fail('Existing release PR has an unexpected source identity');
    }
    if (own.filter(pr => pr.state === 'open').length > 1 || own.filter(pr => pr.merged_at).length > 1) {
      fail('Ambiguous release PR history');
    }
    const main = () => api(`${base}/git/ref/heads/main`).object?.sha;
    const sourceMain = main();
    if (!SHA.test(sourceMain ?? '')) fail('Invalid main identity');
    let accepted;
    const merged = own.find(pr => pr.merged_at);
    if (merged) {
      accepted = merged.merge_commit_sha;
      if (!SHA.test(accepted ?? '')) fail('Invalid merged release identity');
      requireAncestor(api(`${base}/compare/${accepted}...${sourceMain}`), accepted);
    } else {
      const direct = api(`${base}/compare/${sourceMain}...${source}`);
      if (!isAncestor(direct, sourceMain)) {
        // Release merges add a main-only commit, but no new content. A later
        // dev source may contain the previous tested source without containing
        // that merge commit; no synchronization/retest loop is required.
        const priorMain = api(`${base}/git/commits/${sourceMain}`);
        const priorSource = priorMain.parents?.length === 2 ? priorMain.parents[1].sha : undefined;
        if (!SHA.test(priorSource ?? '')) fail('Main contains content outside the tested development lineage');
        requireTreeIdentity(priorMain.tree?.sha, api(`${base}/git/commits/${priorSource}`).tree?.sha);
        requireAncestor(api(`${base}/compare/${priorSource}...${source}`), priorSource);
      }
      let pr = own.find(pr => pr.state === 'open');
      if (!pr) {
        const refs = api(`${base}/git/matching-refs/heads/${branch}`);
        const ref = refs.find(item => item.ref === `refs/heads/${branch}`);
        if (ref && ref.object?.sha !== source) fail('Release branch does not identify the tested source');
        if (!ref) api(`${base}/git/refs`, 'POST', { ref: `refs/heads/${branch}`, sha: source });
        pr = api(`${base}/pulls`, 'POST', {
          title: `Release tested development ${source.slice(0, 12)}`,
          head: branch,
          base: 'main',
          body: `Promote the artifacts proved by integration run ${runId}. Publication verifies the accepted tree against the tested source and retains distinct source and main identities.`,
        });
      }
      // GitHub guards the head SHA. Recheck the base immediately before merging;
      // the post-merge tree check also refuses publication on a base race.
      if (main() !== sourceMain) fail('Main changed during release preparation; rerun publication');
      const verification = api(`${base}/statuses/${source}`, 'POST', {
        state: 'success', context: 'release-verification',
        description: 'Required artifacts, source identity, and release ancestry verified',
        target_url: integration.html_url || `https://github.com/${repository}/actions/runs/${runId}`,
      });
      if (verification.state !== 'success' || verification.context !== 'release-verification') {
        fail('GitHub did not record successful release verification; merge refused');
      }
      const result = api(`${base}/pulls/${pr.number}/merge`, 'PUT', { sha: source, merge_method: 'merge' });
      if (result.merged !== true || !SHA.test(result.sha ?? '')) fail('Release PR was not merged');
      accepted = result.sha;
      if (main() !== accepted) fail('Main advanced during release merge; rerun to verify ancestry');
    }
    requireTreeIdentity(sourceTree, api(`${base}/git/commits/${accepted}`).tree?.sha);

    const staging = await mkdtemp(path.join(os.tmpdir(), 'conduit-publication-'));
    try {
      const assets = [];
      for (const receipt of receipts) {
        const name = `${receipt.target}.tar.gz`;
        const filename = path.join(staging, name);
        // Normalized archive metadata makes an identical bundle resumable after
        // extraction on a different runner; the product bytes are never rebuilt.
        run('tar', ['--sort=name', '--mtime=@0', '--owner=0', '--group=0', '--numeric-owner',
          '-czf', filename, '-C', path.resolve(directory), receipt.target]);
        assets.push({ name, sha256: await digest(filename) });
      }
      const manifest = {
        schema: 'conduit.pipeline-release/v1', repository, integrationRun: String(runId),
        testedSourceSha: source, acceptedMainSha: accepted, treeSha: sourceTree,
        targets: receipts, assets,
      };
      const manifestName = 'manifest.json';
      await writeFile(path.join(staging, manifestName), `${JSON.stringify(manifest, null, 2)}\n`);
      const allAssets = [...assets, { name: manifestName, sha256: await digest(path.join(staging, manifestName)) }];
      const releases = pages(`${base}/releases?per_page=100`);
      let release = releases.find(item => item.tag_name === tag);
      const tagRefs = api(`${base}/git/matching-refs/tags/${tag}`);
      const existingTag = tagRefs.find(item => item.ref === `refs/tags/${tag}`);
      if (existingTag && existingTag.object?.sha !== accepted) fail('Release tag has moved or has a different accepted identity');
      if (!existingTag) {
        if (release && !release.draft) fail('Published release tag is missing');
        api(`${base}/git/refs`, 'POST', { ref: `refs/tags/${tag}`, sha: accepted });
      }
      if (api(`${base}/commits/${tag}`).sha !== accepted) fail('Release tag does not identify accepted main');
      if (release) {
        if (release.prerelease) fail('Existing release has a different publication class');
      } else {
        // The already-verified tag owns the commit identity. target_commitish is
        // unused for an existing tag; omitting a historical value also avoids
        // requesting permission to modify old workflow files on release APIs.
        release = api(`${base}/releases`, 'POST', {
          tag_name: tag, name: `Conduit ${source.slice(0, 12)}`,
          body: `Tested source: ${source}\nAccepted main: ${accepted}\nIdentical tree: ${sourceTree}\nIntegration run: ${runId}\nProof classes are recorded separately for every target in manifest.json.`,
          draft: true, prerelease: false,
        });
      }
      const existingAssets = pages(`${base}/releases/${release.id}/assets?per_page=100`);
      const expectedNames = new Set(allAssets.map(asset => asset.name));
      if (existingAssets.some(asset => !expectedNames.has(asset.name)) ||
          new Set(existingAssets.map(asset => asset.name)).size !== existingAssets.length) {
        fail('Existing release has unexpected or duplicate assets');
      }
      const downloaded = path.join(staging, 'existing');
      await mkdir(downloaded);
      for (const asset of allAssets) {
        if (existingAssets.some(existing => existing.name === asset.name)) {
          run('gh', ['release', 'download', tag, '--repo', repository, '--pattern', asset.name, '--dir', downloaded]);
          if (await digest(path.join(downloaded, asset.name)) !== asset.sha256) {
            fail(`Existing asset differs from the tested bundle: ${asset.name}`);
          }
        } else {
          if (!release.draft) fail('Published release is incomplete; refusing to mutate public assets');
          run('gh', ['release', 'upload', tag, path.join(staging, asset.name), '--repo', repository]);
        }
      }
      if (api(`${base}/commits/${tag}`).sha !== accepted) fail('Release tag changed before publication');
      if (release.draft) {
        // A historical draft may resume after a newer batch has reached main.
        // Retain its artifacts without moving the latest release backward.
        api(`${base}/releases/${release.id}`, 'PATCH', {
          draft: false, make_latest: main() === accepted ? 'true' : 'false',
        });
      }
      if (api(`${base}/commits/${tag}`).sha !== accepted) fail('Published tag does not identify accepted main');
      if (output) await appendFile(output, `main-sha=${accepted}\nsource-sha=${source}\nsite-current=${main() === accepted}\n`);
      return { sourceSha: source, mainSha: accepted, treeSha: sourceTree, tag, manifest };
    } finally {
      await rm(staging, { recursive: true, force: true });
    }
  };
}

export const publish = createPublisher();
