// Preparing a fresh/reloaded realization has a separate finite budget from input.
export const BROWSER_COLD_PREPARATION_MS = 60_000;

export async function waitForWorkspacePreparation(page, { surfaceTitle, expectedState = 'Playing' } = {}) {
  if (!['Playing', 'Idle', 'Lulled'].includes(expectedState)) throw new Error('Unsupported preparation success state');
  const ready = await page.waitForFunction(({ surfaceTitle, expectedState }) => {
    const state = document.querySelector('[data-play-state]')?.textContent?.trim();
    const title = document.querySelector('#surface-title')?.textContent?.trim();
    if (/^(Failed|Refused|Stopped|Cancelled|Completed|Fulfilled)$/.test(state ?? '')) return { state, title };
    return state === expectedState && (!surfaceTitle || title === surfaceTitle) ? { state, title } : false;
  }, { surfaceTitle, expectedState }, { timeout: BROWSER_COLD_PREPARATION_MS });
  try {
    const actual = await ready.jsonValue();
    if (actual.state !== expectedState) throw new Error(`Workspace preparation ended in ${actual.state}`);
  } finally {
    await ready.dispose();
  }
}
