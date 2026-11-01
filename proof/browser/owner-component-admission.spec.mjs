import { expect, test } from '@playwright/test';

// The public SDK must not install an optional Back before its byte/Host gate.
// This exercises real module evaluation, not a mocked element registry.
test('loading the public SDK leaves the optional owner component unregistered', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  expect(await page.evaluate(() => Boolean(customElements.get('conduit-owner-face')))).toBe(false);
  await page.evaluate(() => import('/targets/browser/sdk/browser-sdk.mjs'));
  expect(await page.evaluate(() => Boolean(customElements.get('conduit-owner-face')))).toBe(false);
});

for (const corruption of [null, 'literal', 'computed', 'foreign-blob', 'root-computed']) {
  test(`admitted SDK keeps only declared lazy renderer imports (${corruption ?? 'current'})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const result = await page.evaluate(async corruption => {
      const { admitApplicationSdk } = await import('/targets/browser/host/assets/browser-application-sdk.mjs');
      const originalRevoke = URL.revokeObjectURL;
      const revoked = [];
      URL.revokeObjectURL = url => { revoked.push(url); originalRevoke(url); };
      const sources = {
        root: 'import { renderLater } from "./browser-sdk-plots.mjs"; export const Conduit = { browser: () => ({ renderLater }) };',
        plots: 'export const renderLater = () => import("./browser-sdk-patchbay.mjs").then(module => module.marker);',
        renderer: 'import { prefix } from "./helper.mjs"; customElements.define("fixture-lazy-sdk-renderer", class extends HTMLElement {}); export const marker = `${prefix}-renderer`;',
        helper: 'export const prefix = "admitted";',
        continuity: 'export const applicationContinuity = async () => ({ identity: "fixture/continuity" });',
      };
      if (corruption === 'literal') sources.plots += '\nexport const bad = () => import("./unadmitted.mjs");';
      if (corruption === 'computed') sources.plots += '\nexport const bad = path => import(path);';
      if (corruption === 'foreign-blob') sources.plots += '\nexport const bad = () => import("blob:http://component.test/unadmitted");';
      if (corruption === 'root-computed') sources.root += '\nexport const bad = path => import(path);';
      const resource = (role, file, dependencies = []) => ({ role, path: `sdk/${file}`, kind: 'content',
        url: new URL(`/sdk/${file}`, location.href), dependencies });
      const manifest = { applicationId: 'fixture/sdk', packageDigest: 'fixture/digest',
        stateCompatibility: { identity: 'fixture/continuity', version: 1 }, resources: [
          resource('browser-sdk', 'browser-sdk.mjs', [{ role: 'plots', specifier: './browser-sdk-plots.mjs' }]),
          resource('plots', 'browser-sdk-plots.mjs', [{ role: 'renderer', specifier: './browser-sdk-patchbay.mjs' }]),
          resource('renderer', 'browser-sdk-patchbay.mjs', [{ role: 'helper', specifier: './helper.mjs' }]),
          resource('helper', 'helper.mjs'), resource('continuity', 'browser-sdk-continuity.mjs'),
        ] };
      const bytes = new Map(manifest.resources.map(r => [r.role,
        new TextEncoder().encode(sources[r.role === 'browser-sdk' ? 'root' : r.role])]));
      try {
        const sdk = await admitApplicationSdk(manifest, bytes);
        const eager = Boolean(customElements.get('fixture-lazy-sdk-renderer'));
        // BFCache retains the same document and its not-yet-used lazy graph.
        const beforeCachedLeave = revoked.length;
        dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
        const cachedLeave = revoked.length;
        const marker = await sdk.browser().renderLater();
        const rendered = Boolean(customElements.get('fixture-lazy-sdk-renderer'));
        dispatchEvent(new PageTransitionEvent('pagehide', { persisted: false }));
        return { eager, rendered, marker, beforeCachedLeave, cachedLeave,
          uniqueReleased: new Set(revoked).size, released: revoked.length };
      } catch (error) { return { refusal: error.message }; }
    }, corruption);
    if (corruption) {
      expect(result.refusal).toMatch(/Unexpected SDK dynamic import|SDK lazy dependency was not admitted/);
    } else {
      expect(result).toMatchObject({ eager: false, rendered: true, marker: 'admitted-renderer',
        uniqueReleased: 5, released: 5 });
      expect(result.cachedLeave).toBe(result.beforeCachedLeave);
    }
  });
}
