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

function candidateSummary(run) {
  const { source_sha, ...summary } = runSummary(run);
  return { ...summary, pull_request_head_sha: source_sha,
    tested_merge_preview_sha: null,
    identity_note: 'Actions head_sha identifies the PR head; the tested merge preview requires its checkout/receipt evidence.' };
}

export function selectPublishedRelease(releases) {
  const eligible = releases.filter(r => !r.draft && !r.prerelease && r.assets.some(a => a.name === 'manifest.json'));
  if (eligible.some(r => !Number.isFinite(Date.parse(r.published_at)))) throw new Error('unknown release publication date');
  return eligible.sort((a, b) => Date.parse(b.published_at) - Date.parse(a.published_at))[0] ?? null;
}
function publicationSummary(run) {
  if (!run) return null;
  const { source_sha, ...summary } = runSummary(run);
  return { ...summary, workflow_head_sha: source_sha, tested_source_sha: null };
}
function livePagesSummary(pages) {
  if (!pages) return null;
  if (pages.manifest.schema !== 'conduit.site-publication/v1') throw new Error('unknown Pages publication manifest');
  return { source_sha: exact(pages.manifest.sourceCommit), manifest_url: pages.url };
}

/** Observations only: neither promotion nor deployment is inferred from a green run. */
export function buildDeliveryStatus({ repository, development, main, integrations, candidates, publications, release, deployments, pages }) {
  exact(development); exact(main);
  integrations = integrations.filter(run => run.head_branch === 'dev');
  const integration = integrations.find(run => run.head_sha === development);
  const green = integrations.find(run => run.status === 'completed' && run.conclusion === 'success');
  const published = release ? { tested_source_sha: exact(release.testedSourceSha),
    accepted_main_sha: exact(release.acceptedMainSha), integration_run: release.integrationRun,
    tree_sha: exact(release.treeSha) } : null;
  const livePages = livePagesSummary(pages);
  const deployed = deployments.find(item => item.statuses[0]?.state === 'success');
  return { schema: 'conduit.delivery-status/v1', repository, observed_at: new Date().toISOString(),
    admitted_dev_sha: development, accepted_main_sha: main,
    current_integration: runSummary(integration), latest_green_integration: runSummary(green),
    published_release: published, latest_publication_attempt: publicationSummary(publications[0]),
    latest_pages_attempt: deployments[0] ? { workflow_head_sha: exact(deployments[0].sha), source_sha: null, statuses: deployments[0].statuses } : null,
    latest_successful_pages_deployment: deployed ? { workflow_head_sha: exact(deployed.sha), source_sha: null, url: deployed.statuses[0].environment_url } : null,
    live_pages_publication: livePages,
    differences: { dev_from_green: green ? development !== green.head_sha : null,
      dev_from_published: published ? development !== published.tested_source_sha : null,
      main_from_published: published ? main !== published.accepted_main_sha : null,
      dev_from_deployed_pages: livePages ? development !== livePages.source_sha : null },
    queue: { integrations: integrations.filter(r => r.status !== 'completed').map(runSummary),
      publications: publications.filter(r => r.status !== 'completed').map(publicationSummary) },
    timing_baseline: { scope: 'bounded Actions sample; duration from Actions run_started_at to updated_at; includes setup, excludes pre-start queue, and is not billed runner cost',
      candidates: candidates.map(candidateSummary), integrations: integrations.map(runSummary), publications: publications.map(publicationSummary) },
    observation_limit: 'Most recent 100 runs per workflow 100 release records (ordered by explicit publication date) and 20 Pages deployments; absent evidence remains null.' };
}

export async function readDeliveryStatus(repository) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error('expected owner/repository');
  function api(endpoint) {
    const result = spawnSync('gh', ['api', endpoint], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, timeout: 30_000 });
    if (result.error || result.status !== 0) throw new Error(`delivery observation failed for ${endpoint}`);
    return JSON.parse(result.stdout);
  }
  const base = `repos/${repository}`;
  const runs = name => api(`${base}/actions/workflows/${name}.yml/runs?per_page=100`).workflow_runs;
  const latest = selectPublishedRelease(api(`${base}/releases?per_page=100`));
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
  let pages = null;
  if (deployments.some(item => item.statuses[0]?.state === 'success')) {
    const site = api(`${base}/pages`).html_url;
    const url = new URL('site-publication.json', site);
    if (url.protocol !== 'https:' || url.username || url.password) throw new Error('invalid configured Pages URL');
    const response = await fetch(url, { signal: AbortSignal.timeout(30_000), cache: 'no-store' });
    if (!response.ok) throw new Error('could not observe live Pages publication');
    const body = await response.text();
    if (body.length > 1024 * 1024) throw new Error('oversized Pages publication manifest');
    pages = { url: url.href, manifest: JSON.parse(body) };
  }
  return buildDeliveryStatus({ repository, development: api(`${base}/git/ref/heads/dev`).object.sha,
    main: api(`${base}/git/ref/heads/main`).object.sha, integrations: runs('integration'),
    candidates: runs('candidate').slice(0, 5).map(run => ({ ...run, jobs: api(`${base}/actions/runs/${run.id}/jobs?per_page=100`).jobs })), publications: runs('publish'), release, deployments, pages });
}
