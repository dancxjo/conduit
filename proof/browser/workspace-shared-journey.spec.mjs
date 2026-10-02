import { expect, test } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { startStaticProduct } from './static-product-server.mjs';

const journey = JSON.parse(readFileSync(new URL('../journeys/workspace.json', import.meta.url), 'utf8'));
let entrance;
test.beforeEach(async () => { entrance = await startStaticProduct('target/workspace-product', '/conduit/workspace/'); });
test.afterEach(() => entrance?.child.kill());

// The action sequence and user-facing explanation are also consumed by the
// QMP driver. This adapter only supplies browser interactions and observations.
test('the shared Workspace journey from zero Body through explicit rest and finish', async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  const current = () => page.evaluate(() => globalThis.__conduitWorkspace.current());
  const tutorial = page.locator('[data-body-tutorial]');
  const observations = [];
  let bodyId;
  for (const action of journey.actions) {
    await test.step(action.description, async () => {
      switch (action.id) {
        case 'arrive':
          await page.goto(entrance.url);
          await expect(page.getByRole('heading', { name: 'A body of your own', exact: true })).toBeVisible();
          break;
        case 'configure-birth': {
          const birth = page.locator('.body-birth-runner');
          await birth.getByLabel('Friendly Body name', { exact: true }).fill(journey.name);
          await birth.getByRole('checkbox', { name: 'Startup Chime', exact: true }).uncheck();
          for (const title of journey.plots) await birth.getByRole('checkbox', { name: title, exact: true }).check();
          await expect(birth.getByRole('checkbox', { checked: true })).toHaveCount(journey.plots.length);
          break;
        }
        case 'birth':
          await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
          await expect(page.locator('[data-play-state]')).toHaveText('Lulled');
          await expect(page.locator('[data-body-name]')).toHaveText(journey.name);
          bodyId = (await current()).body_id;
          expect(bodyId).toBeTruthy();
          await expect(tutorial).toContainText('Wake this body');
          break;
        case 'wake':
          await tutorial.getByRole('button', { name: 'Wake the retained body', exact: true }).click();
          await expect(page.locator('[data-play-state]')).toHaveText('Playing');
          break;
        case 'use-current':
          await tutorial.getByRole('button', { name: 'Try the current Plot', exact: true }).click();
          await page.keyboard.type(journey.input);
          await expect(page.locator('[data-plot-output] output:visible')).toHaveText(journey.input);
          break;
        case 'tutorial':
          await page.getByRole('navigation', { name: 'Your plots' }).getByRole('button', { name: 'Tutorial', exact: true }).click();
          await expect(tutorial).toContainText('Try a Plot, inspect what happened, then try again.');
          break;
        case 'read-all': {
          const lifecycle = tutorial.getByRole('status').filter({ hasText: /^Lifecycle evidence —/ });
          await lifecycle.scrollIntoViewIfNeeded();
          await expect(lifecycle).toBeVisible();
          await expect(lifecycle).toContainText('Not yet fulfilled');
          break;
        }
        case 'lull':
          await page.getByRole('button', { name: 'lull body', exact: true }).click();
          await expect(page.locator('[data-play-state]')).toHaveText('Lulled');
          break;
        case 'fulfill': {
          const confirmation = page.waitForEvent('dialog').then(async dialog => {
            expect(dialog.type()).toBe('confirm');
            expect(dialog.message()).toContain('Finish this body permanently?');
            await dialog.accept();
          });
          await Promise.all([
            confirmation,
            page.getByRole('button', { name: 'Finish body', exact: true }).click(),
          ]);
          await expect(page.locator('[data-play-state]')).toHaveText('Fulfilled');
          await expect(tutorial).toContainText('This Body is fulfilled. Inspect its retained biography');
          await expect(tutorial).not.toContainText('Not yet fulfilled');
          break;
        }
        default: throw new Error(`Unsupported shared Workspace action: ${action.id}`);
      }
      const state = bodyId ? await current() : undefined;
      if (bodyId) expect(state.body_id).toBe(bodyId);
      if (state && state.state !== 'AWAKE') {
        await expect(page.locator('#plot-input')).toHaveAttribute('aria-disabled', 'true');
        await expect(page.locator('#plot-input .input-prompt')).toBeHidden();
        await expect(page.locator('[data-surface-invitation]')).toHaveText(
          await page.locator('#surface-guidance').innerText(),
        );
      }
      let mask;
      if (state?.plan_id) {
        mask = await page.evaluate(() => globalThis.__conduitWorkspace.maskObservation());
        expect(mask.mask_show.show.lifecycle).toBe('Available');
        expect(mask.mask_show.presentation_id).toBe(mask.presentation.identity);
        expect(mask.mask_show.presentation_revision).toBe(mask.presentation.revision);
        expect(mask.mask_show.presentation_plan_id).toBe(state.plan_id);
        await expect(tutorial).toHaveAttribute('data-mask-show-id', mask.mask_show.show_id);
        await expect(tutorial).toHaveAttribute('data-mask-plan-id', mask.planned_mask.plan.plan_id);
        await expect(tutorial).toHaveAttribute('data-mask-play-id', mask.mask_play.active_play_id);
        if (action.id === 'use-current') {
          expect(mask.interaction.semantic_action.identity).toBe('body.use-current');
          expect(mask.interaction.correlation.show_id).toBe(mask.mask_show.show_id);
          expect(mask.interaction.correlation.presentation_revision).toBe(mask.presentation.revision);
        }
      }
      observations.push({ action: action.id, state, mask });
      await page.screenshot({ path: testInfo.outputPath(`${action.capture}.png`), fullPage: true });
    });
  }
  writeFileSync(testInfo.outputPath('workspace-proof.json'), JSON.stringify({
    schema: 'conduit.browser/workspace-journey-proof@1',
    contract: journey.schema,
    observations,
    input: 'real-browser-controls-and-keyboard',
    shared_three_host_body_observed: false,
    screen_free_interaction_observed: false,
    human_observation: false,
  }, null, 2));
});
