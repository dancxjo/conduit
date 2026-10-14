import { spawnSync } from 'node:child_process';

const SHA = /^[a-f0-9]{40}$/;
function exact(value) {
  if (!SHA.test(value ?? '')) throw new Error('delivery status requires exact commit identities');
  return value;
}
function runSummary(run) {
  if (!run) return null;
  const completed = run.status === 'completed';
  return { id: run.id, branch: run.head_branch, event: run.event, attempt: run.run_attempt, source_sha: exact(run.head_sha), status: run.status,
    conclusion: completed ? run.conclusion : null, url: run.html_url,
    selected_jobs: run.jobs?.filter(job => job.conclusion !== 'skipped').map(job => ({ name: job.name, status: job.status, conclusion: job.conclusion, url: job.html_url })) ?? null,
    duration_seconds: completed ? Math.max(0, (Date.parse(run.updated_at) - Date.parse(run.run_started_at)) / 1000) : null };
}

/** Observations only: neither promotion nor deployment is inferred from a green run. */
export function buildDeliveryStatus({ repository, development, main, integrations, candidates, publications, release, deployments }) {
  exact(development); exact(main);
  integrations = integrations.filter(run => run.head_branch === 'dev');
  const integration = integrations.find(run => run.head_sha === development);
  const green = integrations.find(run => run.status === 'completed' && run.conclusion === 'success');
  const published = release ? { tested_source_sha: exact(release.testedSourceSha),
    accepted_main_sha: exact(release.acceptedMainSha), integration_run: release.integrationRun,
    tree_sha: exact(release.treeSha) } : null;
  const deployed = deployments.find(item => item.statuses[0]?.state === 'success');
  return { schema: 'conduit.delivery-status/v1', repository, observed_at: new Date().toISOString(),
    admitted_dev_sha: development, accepted_main_sha: main,
    current_integration: runSummary(integration), latest_green_integration: runSummary(green),
    published_release: published, latest_publication_attempt: runSummary(publications[0]),
    latest_pages_attempt: deployments[0] ? { source_sha: exact(deployments[0].sha), statuses: deployments[0].statuses } : null,
    latest_successful_pages_deployment: deployed ? { source_sha: exact(deployed.sha), url: deployed.statuses[0].environment_url } : null,
    differences: { dev_from_green: green ? development !== green.head_sha : null,
      dev_from_published: published ? development !== published.tested_source_sha : null,
      main_from_published: published ? main !== published.accepted_main_sha : null,
      dev_from_deployed_pages: deployed ? development !== deployed.sha : null },
    queue: { integrations: integrations.filter(r => r.status !== 'completed').map(runSummary),
      publications: publications.filter(r => r.status !== 'completed').map(runSummary) },
    timing_baseline: { scope: 'bounded Actions sample; duration from Actions run_started_at to updated_at; includes setup, excludes pre-start queue, and is not billed runner cost',
      candidates: candidates.map(runSummary), integrations: integrations.map(runSummary), publications: publications.map(runSummary) },
    observation_limit: 'Most recent 100 runs per workflow and 20 Pages deployments; absent evidence remains null.' };
}

export function readDeliveryStatus(repository) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error('expected owner/repository');
  function api(endpoint) {
    const result = spawnSync('gh', ['api', endpoint], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, timeout: 30_000 });
    if (result.error || result.status !== 0) throw new Error(`delivery observation failed for ${endpoint}`);
    return JSON.parse(result.stdout);
  }
  const base = `repos/${repository}`;
  const runs = name => api(`${base}/actions/workflows/${name}.yml/runs?per_page=100`).workflow_runs;
  const releases = api(`${base}/releases?per_page=20`).filter(r => !r.draft && !r.prerelease);
  const latest = releases.find(r => r.assets.some(a => a.name === 'manifest.json'));
  let release = null;
  if (latest) {
    const asset = latest.assets.find(a => a.name === 'manifest.json');
    const result = spawnSync('gh', ['api', asset.url, '-H', 'Accept: application/octet-stream'], { encoding: 'utf8', maxBuffer: 1024 * 1024, timeout: 30_000 });
    if (result.error || result.status !== 0) throw new Error('could not read published release manifest');
    release = JSON.parse(result.stdout);
    if (release.schema !== 'conduit.pipeline-release/v1' || release.repository !== repository) throw new Error('foreign release manifest');
  }
  const deployments = api(`${base}/deployments?environment=github-pages&per_page=20`).map(item => ({ ...item,
    statuses: api(`${base}/deployments/${item.id}/statuses?per_page=100`).map(({state, environment_url, created_at}) => ({state, environment_url, created_at})) }));
  return buildDeliveryStatus({ repository, development: api(`${base}/git/ref/heads/dev`).object.sha,
    main: api(`${base}/git/ref/heads/main`).object.sha, integrations: runs('integration'),
    candidates: runs('candidate').slice(0, 5).map(run => ({ ...run, jobs: api(`${base}/actions/runs/${run.id}/jobs?per_page=100`).jobs })), publications: runs('publish'), release, deployments });
}
