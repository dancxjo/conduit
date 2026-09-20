import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { startPresenceProbe } from './browser-presence-support.mjs';
import { startStaticProduct } from './tour-test-server.mjs';

let entrance;
let temporary;
const processes = [];

test.beforeEach(async () => {
  entrance = await startStaticProduct('target/workspace-product', '/conduit/workspace/');
});
test.afterEach(async () => {
  for (const process of processes.splice(0)) if (process.exitCode === null) process.kill('SIGTERM');
  if (temporary) await rm(temporary, { recursive: true, force: true });
  temporary = null;
  entrance?.child.kill();
});

async function openPatchbay(page, evidencePath, probeUrl) {
  const patchbay = spawn('target/debug/patchbay-html', [
    '--body-evidence', evidencePath, '--external-reader', '--body-invitation', probeUrl,
    '--form', 'button-across-room', 'forms/button-across-room/main.conduit',
    '--form', 'little-seismograph-display', 'forms/little-seismograph/main.conduit',
  ], { cwd: new URL('../..', import.meta.url).pathname, stdio: ['ignore', 'pipe', 'pipe'] });
  processes.push(patchbay);
  let output = '';
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Workspace measurement Patchbay start timed out: ${output}`)), 10_000);
    const inspect = chunk => {
      output += chunk;
      const match = output.match(/PATCHBAY_HTML_URL=(http:\/\/127\.0\.0\.1:\d+)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    patchbay.stdout.on('data', inspect);
    patchbay.stderr.on('data', inspect);
    patchbay.once('exit', code => { clearTimeout(timeout); reject(new Error(`Patchbay exited (${code}): ${output}`)); });
  });
  await page.goto(url);
  await expect(page.locator('#body-summary')).toHaveText(/^Body: /);
  return { patchbay, url };
}

const snapshot = page => page.request.get(`${page.url()}/api/snapshot`).then(response => response.json());

test('Workspace-origin Body retains bounded measurement plot, threshold, and history evidence', async ({ page }) => {
  temporary = await mkdtemp(join(tmpdir(), 'conduit-workspace-measurement-'));
  await page.goto(entrance.url);
  const birth = page.locator('.body-birth-runner');
  for (const checkbox of await birth.getByRole('checkbox').all()) {
    if (await checkbox.isChecked()) await checkbox.uncheck();
  }
  await birth.getByRole('checkbox', { name: 'Button Across the Room', exact: true }).check();
  await birth.getByRole('checkbox', { name: 'Little Seismograph', exact: true }).check();
  await birth.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Lulled');
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());

  const evidence = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence);
  expect(evidence.body.workset.forms.map(form => form.name).sort()).toEqual([
    'button_across_room', 'little-seismograph-display',
  ].sort());
  const bodyId = evidence.body_id;
  const evidencePath = join(temporary, 'workspace-measurement.json');
  await writeFile(evidencePath, JSON.stringify(evidence));

  const probe = await startPresenceProbe(['--body-evidence', evidencePath]);
  processes.push(probe.process);
  const { url } = await openPatchbay(page, evidencePath, probe.url);
  const patchbaySnapshot = () => snapshot(page);
  await page.locator('#body-summary').click();
  await page.getByRole('button', { name: 'Join this Body', exact: true }).click();
  await expect(page.locator('#body-membership-status')).toHaveText('Browser membership: admitted', { timeout: 10_000 });
  await expect.poll(async () => (await patchbaySnapshot()).body_host_offer_evidence?.stage, { timeout: 10_000 }).toBe('AdmittedMembership');
  await page.getByRole('button', { name: 'Request active Form evidence', exact: true }).click();
  await expect(page.locator('#body-capability-evidence-status')).toContainText('SelfReported evidence');
  await page.getByRole('button', { name: 'Plan active Forms on this Host', exact: true }).click();
  await expect(page.locator('#body-capability-evidence-status')).toContainText('Body replanned');
  const proposal = await page.request.get(`${url}/api/body-execution-proposal`).then(response => response.json());
  expect(proposal.plan.body_id).toBe(bodyId);
  expect(proposal.plan.forms).toHaveLength(2);

  await page.getByRole('button', { name: 'Start proposed Body Play', exact: true }).click();
  await expect(page.locator('#body-execution-status')).toContainText('Body Play running', { timeout: 10_000 });
  await expect(page.locator('[data-presentation-kind="presentation/measurement-plot"]')).toHaveText('plot 1 samples · 0 omitted');
  await expect(page.locator('[data-presentation-kind="presentation/measurement-threshold"]')).toHaveText('threshold Above · Some(RoseAbove)');
  await page.getByRole('button', { name: 'Cancel Body Play', exact: true }).click();
  await expect(page.locator('#body-execution-status')).toContainText('Body Play cancelled', { timeout: 10_000 });
  const terminal = await patchbaySnapshot();
  expect(terminal.body_planning.body_id).toBe(bodyId);
  expect(terminal.body_planning.execution_claims[0].phase.Terminal.disposition).toBe('cancelled');
});
