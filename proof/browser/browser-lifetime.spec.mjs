import { writeFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';

test('actual browser Host retires A and refuses its exact effects while B remains live', async ({ page }, testInfo) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.clock.install();
  await page.clock.pauseAt(new Date());
  await page.goto('/proof/browser/browser-lifetime.test.html');
  await expect(page.locator('#status')).toHaveText('ready');
  await page.waitForFunction(() => __browserLifetime.ready());
  await page.locator('#surface').click({ position: { x: 10, y: 10 } });
  const replaced = await page.evaluate(() => __browserLifetime.replace());
  expect(replaced.close.receipt.disposition).toBe('cancelled');
  expect(replaced.close.receipt.active_play_id).toBe(replaced.oldPlay.active_play_id);
  expect(replaced.newPlay.active_play_id).not.toBe(replaced.oldPlay.active_play_id);
  await page.waitForFunction(() => __browserLifetime.ready());
  // Establish B's independent input-driven sibling before testing no-effect.
  await page.locator('#surface').click({ position: { x: 10, y: 10 } });
  await expect.poll(async () => (await page.evaluate(() => __browserLifetime.snapshot())).outputs.filter(output => output.kind !== 'face/count').length).toBeGreaterThan(0);
  await expect(page.locator('[data-face-kind="face/indicator-state"]')).toHaveText('Button state: false');
  const baseline = await page.evaluate(() => __browserLifetime.settledSnapshot());
  const replay = await page.evaluate(() => __browserLifetime.replay());
  expect(replay.before).toEqual(baseline);
  expect(replay.after).toEqual(baseline);
  // A separate CDP invocation plus an actual MessageChannel task fence drains
  // the old promises before comparing against the original settled baseline.
  const afterCallbacks = await page.evaluate(() => __browserLifetime.settledSnapshot());
  expect(afterCallbacks).toEqual(baseline);
  expect(replay.callbacks).toBeGreaterThan(0);
  expect(replay.inputCallbacks).toBeGreaterThan(0);
  for (const result of replay.results) {
    expect(result.status).toBeLessThan(0);
    expect(result.stagingCleared).toBe(true);
    expect(result.refusal).toMatchObject({ schema: 'conduit.browser/completion-refusal@2', disposition: 'refused', active_play_id: replaced.newPlay.active_play_id, kernel_failure_code: null, kernel_failure_detail: null });
  }
  const beforeTimer = await page.evaluate(() => __browserLifetime.snapshot());
  await page.clock.runFor(10001);
  const afterTimer = await page.evaluate(() => __browserLifetime.snapshot());
  expect(afterTimer.play).toEqual(beforeTimer.play);
  expect(afterTimer.html).not.toEqual(beforeTimer.html);
  const sibling = beforeTimer.outputs.filter(output => output.kind !== 'face/count');
  expect(sibling.length).toBeGreaterThan(0);
  expect(afterTimer.outputs.filter(output => output.kind !== 'face/count')).toEqual(sibling);
  await page.locator('#surface').hover({ position: { x: 10, y: 10 } });
  await page.mouse.down();
  await expect.poll(async () => (await page.evaluate(() => __browserLifetime.snapshot())).outputs.filter(output => output.kind !== 'face/count')).not.toEqual(sibling);
  await page.mouse.up();
  const state = await page.evaluate(() => __browserLifetime.state());
  expect(state.errors).toEqual([]);
  expect(errors).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath('browser-lifetime.png'), fullPage: true });
  const receipt = JSON.stringify({ replaced, baseline, replay, afterCallbacks, beforeTimer, afterTimer, state }, null, 2);
  await writeFile(testInfo.outputPath('lifetime-receipt.json'), receipt);
  await testInfo.attach('lifetime-receipt', { body: receipt, contentType: 'application/json' });
  expect((await page.evaluate(() => __browserLifetime.close())).receipt.disposition).toBe('cancelled');
});
