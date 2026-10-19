import { expect, test } from '@playwright/test';
import { waitForWorkspacePreparation } from './browser-preparation.mjs';

test('cold preparation can finish after the ordinary five-second response budget', async ({ page }) => {
  await page.setContent('<p data-play-state>Playing</p><h1 id="surface-title">Previous plot</h1>');
  await page.evaluate(() => setTimeout(() => {
    document.querySelector('[data-play-state]').textContent = 'Playing';
    document.querySelector('#surface-title').textContent = 'Requested plot';
  }, 5_200));
  await waitForWorkspacePreparation(page, { surfaceTitle: 'Requested plot' });
  await expect(page.locator('#surface-title')).toHaveText('Requested plot');
});

test('refused or prematurely completed preparation fails promptly', async ({ page }) => {
  test.setTimeout(5_000);
  for (const state of ['Refused', 'Completed']) {
    await page.setContent(`<p data-play-state>${state}</p>`);
    await expect(waitForWorkspacePreparation(page)).rejects.toThrow(`Workspace preparation ended in ${state}`);
  }
});

test('ordinary output assertions retain their five-second deadline after preparation', async ({ page }) => {
  await page.setContent('<p data-play-state>Playing</p><output id="result">pending</output>');
  await waitForWorkspacePreparation(page);
  await expect(expect(page.locator('#result')).toHaveText('ready')).rejects.toThrow(/Timeout:\s+5000ms/);
});

// Idle is an explicit success only for a restored sound-only Body.
test('sound-only restoration waits for Idle and still rejects terminal failure', async ({ page }) => {
  await page.setContent('<p data-play-state>Preparing</p>');
  await page.evaluate(() => setTimeout(() => {
    document.querySelector('[data-play-state]').textContent = 'Idle';
  }, 5_200));
  await waitForWorkspacePreparation(page, { expectedState: 'Idle' });
  await expect(page.locator('[data-play-state]')).toHaveText('Idle');
  await page.setContent('<p data-play-state>Refused</p>');
  await expect(waitForWorkspacePreparation(page, { expectedState: 'Idle' }))
    .rejects.toThrow('Workspace preparation ended in Refused');
  await expect(waitForWorkspacePreparation(page, { expectedState: 'Refused' }))
    .rejects.toThrow('Unsupported preparation success state');
});
