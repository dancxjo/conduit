import { expect, test } from '@playwright/test';

// Real Handbook orchestration and renderers over a deferred fixture SDK.
// These observations are not Body action receipts or physical input proof.
async function handbook(page, view, selected) {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(async ({ view, selected }) => {
    const { startOwnerParticipation } = await import('/targets/browser/handbook/owner-participation.mjs');
    const { OwnerFaceElement } = await import('/targets/browser/handbook/owner-face-component.mjs');
    const response = await fetch('/targets/browser/handbook/owner-face-look.css');
    if (!response.ok) throw new Error('Fixture Look unavailable');
    const look = new Uint8Array(await response.arrayBuffer());
    document.body.innerHTML = `<header class="handbook-introduction"><h1></h1><p class="lede"></p></header>
      <a class="handbook-start"></a><div class="handbook-first-steps"></div>
      <div class="handbook-next"></div><div class="handbook-local-note"></div><main></main>`;
    let revision = 0, available = true, onState;
    const fixture = globalThis.busyFixture = { calls: [], admitted: true, pause: null, refuse: null };
    const wait = async stage => {
      if (fixture.pause === stage) {
        fixture.waiting = stage;
        await new Promise(resolve => {
          fixture.complete = () => { fixture.pause = null; fixture.waiting = null; resolve(); };
        });
      }
      if (fixture.refuse === stage) throw new Error(`Fixture ${stage} refused`);
    };
    const shown = { ...view, body_id: 'body/fixture', face_revision: '1',
      mask_renderer_artifact_id: selected ? 'artifact/fixture-component' : 'artifact/browser-runtime',
      mask_plot_id: 'fixture/mask', mask_plan_id: 'fixture/plan', mask_play_id: 'fixture/play',
      route: { plan_id: 'fixture/route', owner_host_id: 'host/owner', owner_boot_id: 'boot/owner',
        mask_host_id: 'host/browser', mask_boot_id: 'boot/browser' } };
    const participation = {
      hostId: 'host/browser', bootId: 'boot/browser',
      presenceState: () => available ? 'available' : 'unavailable',
      membershipCredential: () => ({ body_id: shown.body_id }), biographyEvidence: () => null,
      discardOwnerFaceMask: () => {},
      prepareOwnerFaceMask: async () => {
        const current = ++revision;
        await wait('prepare');
        return { ...shown, face_revision: String(current), show_id: `fixture/show/${current}`,
          show_state: 'prepared', interactions_admitted: false };
      },
      acknowledgeOwnerFaceMask: async prepared => {
        await wait('acknowledge');
        return { ...prepared, show_state: 'available', interactions_admitted: fixture.admitted };
      },
      submitOwnerFaceInteraction: async call => {
        fixture.calls.push(call);
        await wait('submit');
        return { accepted: true };
      },
      inspectOwnerWardrobe: async () => { throw new Error('No fixture wardrobe'); },
      selectedDirectSpeechStart: async () => ({ outcome: 'started', operation_id: 'fixture/speech' }),
    };
    fixture.offline = () => { available = false; onState('offline'); };
    fixture.online = () => { available = true; onState('admitted'); };
    await startOwnerParticipation({ bytes: () => look,
      storage: { readJson: async () => null, writeJson: async () => {} },
      browser: async () => ({ id: 'host/browser', bootId: 'boot/browser',
        admitOwnerFaceComponent: async () => selected
          ? { selected: true, artifactId: 'artifact/fixture-component', constructor: OwnerFaceElement }
          : { selected: false },
        admissionIdentity: () => ({ hostId: 'host/browser', bootId: 'boot/browser', verifyingKey: [] }),
        participate: async options => { onState = options.onState; return participation; } }),
    }, document.querySelector('main'));
  }, { view, selected });
  await page.getByLabel('Body ID', { exact: true }).fill('body/fixture');
  await page.getByLabel('Owner window URL', { exact: true }).fill('ws://127.0.0.1:41000/conduit');
  await page.getByRole('button', { name: 'Join this Body', exact: true }).click();
  await expect(page.locator('main')).toHaveAttribute('data-owner-face-shown', 'true');
  const owner = page.locator('[data-owner-face-surface]');
  await expect(owner).toHaveJSProperty('tagName', selected ? 'CONDUIT-OWNER-FACE' : 'DIV');
  await expect(owner).toHaveAttribute('aria-busy', 'false');
  return owner;
}

export function registerOwnerFaceBusyTests(view) {
  for (const selected of [false, true]) {
    const renderer = selected ? 'selected component' : 'fallback';
    test(`stable unavailable and view-only Faces are idle (${renderer})`, async ({ page }) => {
      const owner = await handbook(page, view, selected);
      for (const show_state of ['available', 'unavailable']) {
        await owner.evaluate((element, show_state) => {
          element.show({ ...globalThis.__conduitOwnerParticipation.face(),
            show_state, interactions_admitted: false }, { invoke: async () => {} });
        }, show_state);
        await expect(owner.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
        await expect(owner).toHaveAttribute('aria-busy', 'false');
      }
    });

    for (const stage of ['prepare', 'acknowledge']) {
      test(`Face ${stage} is busy through disabled presentation (${renderer})`, async ({ page }) => {
        const owner = await handbook(page, view, selected);
        await page.evaluate(stage => { globalThis.busyFixture.pause = stage; }, stage);
        await page.locator('[data-owner-face-refresh]').click();
        await expect.poll(() => page.evaluate(() => globalThis.busyFixture.waiting)).toBe(stage);
        await expect(owner).toHaveAttribute('aria-busy', 'true');
        await expect(owner.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
        await page.evaluate(() => globalThis.busyFixture.complete());
        await expect(owner).toHaveAttribute('aria-busy', 'false');
        await expect(owner.getByRole('button', { name: 'Complete task', exact: true })).toBeEnabled();
        expect(await page.evaluate(() => globalThis.busyFixture.calls)).toEqual([]);
      });
    }

    for (const input of ['pointer', 'keyboard']) {
      for (const refused of [false, true]) {
        test(`${input} action pending then ${refused ? 'refused' : 'accepted'} is truthful (${renderer})`, async ({ page }) => {
          const owner = await handbook(page, view, selected);
          const current = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
          await page.evaluate(refused => {
            globalThis.busyFixture.pause = 'submit';
            globalThis.busyFixture.refuse = refused ? 'submit' : null;
          }, refused);
          const activate = owner.getByRole('button', { name: 'Complete task', exact: true });
          if (input === 'keyboard') {
            await activate.focus();
            await page.keyboard.press('Enter');
          } else {
            await activate.click();
          }
          await expect(owner).toHaveAttribute('aria-busy', 'true');
          await expect(owner.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
          expect(await page.evaluate(() => globalThis.busyFixture.calls)).toEqual([
            { view: current, actionId: view.actions[0].identity, target: view.actions[0].target,
              arguments: [], sequence: 1 },
          ]);
          await page.evaluate(() => globalThis.busyFixture.complete());
          await expect(page.locator('[data-owner-action-result]')).toContainText(refused ? 'refused' : 'accepted');
          await expect(owner).toHaveAttribute('aria-busy', 'false');
          await expect(owner.getByRole('button', { name: 'Complete task', exact: true })).toBeEnabled();
          expect(await page.evaluate(() => globalThis.busyFixture.calls.length)).toBe(1);
        });
      }
    }

    test(`terminal preparation refusal leaves disabled controls idle (${renderer})`, async ({ page }) => {
      const owner = await handbook(page, view, selected);
      await page.evaluate(() => { globalThis.busyFixture.refuse = 'prepare'; });
      await page.locator('[data-owner-face-refresh]').click();
      await expect(page.locator('[data-owner-face-status]')).toContainText('Fixture prepare refused');
      await expect(owner).toHaveAttribute('aria-busy', 'false');
      await expect(owner.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
    });

    test(`route loss clears busy and late completion cannot revive agency (${renderer})`, async ({ page }) => {
      const owner = await handbook(page, view, selected);
      await page.evaluate(() => { globalThis.busyFixture.pause = 'prepare'; });
      await page.locator('[data-owner-face-refresh]').click();
      await expect.poll(() => page.evaluate(() => globalThis.busyFixture.waiting)).toBe('prepare');
      await expect(owner).toHaveAttribute('aria-busy', 'true');
      await page.evaluate(() => globalThis.busyFixture.offline());
      await expect(owner).toHaveAttribute('aria-busy', 'false');
      await expect(owner.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
      await page.evaluate(() => globalThis.busyFixture.online());
      await expect(owner).toHaveAttribute('aria-busy', 'false');
      await page.evaluate(() => globalThis.busyFixture.complete());
      await expect(page.locator('[data-owner-face-status]')).toContainText('route changed');
      await expect(owner).toHaveAttribute('aria-busy', 'false');
      await expect(owner.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
      await expect(page.locator('main')).not.toHaveAttribute('data-owner-face-shown', 'true');
      expect(await page.evaluate(() => globalThis.busyFixture.calls)).toEqual([]);
      await page.locator('[data-owner-face-refresh]').click();
      await expect(page.locator('main')).toHaveAttribute('data-owner-face-shown', 'true');
      await expect(owner).toHaveAttribute('aria-busy', 'false');
      await expect(owner.getByRole('button', { name: 'Complete task', exact: true })).toBeEnabled();
    });

    test(`speech ownership disables agency without marking the Face busy (${renderer})`, async ({ page }) => {
      const owner = await handbook(page, view, selected);
      await page.locator('[data-owner-speech-start]').click();
      await expect(page.locator('[data-owner-speech-result]')).toContainText('started reading');
      await expect(owner.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
      await expect(owner).toHaveAttribute('aria-busy', 'false');
      expect(await page.evaluate(() => globalThis.busyFixture.calls)).toEqual([]);
    });

    test(`retiring a pending surface clears its busy presentation (${renderer})`, async ({ page }) => {
      const owner = await handbook(page, view, selected);
      await page.evaluate(() => { globalThis.busyFixture.pause = 'prepare'; });
      await page.locator('[data-owner-face-refresh]').click();
      await expect.poll(() => page.evaluate(() => globalThis.busyFixture.waiting)).toBe('prepare');
      await expect(owner).toHaveAttribute('aria-busy', 'true');
      await owner.evaluate(element => { globalThis.retiredBusyFace = element; element.remove(); });
      await expect.poll(() => page.evaluate(() => globalThis.retiredBusyFace.getAttribute('aria-busy'))).toBe('false');
      await page.evaluate(() => globalThis.busyFixture.complete());
      await expect(page.locator('main')).toHaveAttribute('data-owner-face-shown', 'true');
      await expect(owner).toHaveAttribute('aria-busy', 'false');
      expect(await page.evaluate(() => globalThis.busyFixture.calls)).toEqual([]);
    });
  }
}
