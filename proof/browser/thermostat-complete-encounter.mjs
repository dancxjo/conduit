// The concise cross-Mask journey. The comprehensive control/bounds proof remains separate.
import assert from 'node:assert/strict';
import path from 'node:path';
import { captureDecimal } from './exact-capture-json.mjs';
import { retainSpokenArtifacts } from './thermostat-spoken-artifacts.mjs';

export async function captureOwnerViewport(page, filename) {
  await page.locator('conduit-owner-face').evaluate(element => element.scrollIntoView({ block: 'start', behavior: 'instant' }));
  await page.screenshot({ path: filename, animations: 'disabled' });
}

export async function completeEncounter({ page, peer, peerState, state, output, owner, ownerFace, ownerStatus,
  currentView, refresh, observe, targetWording, action, raise, wardrobe, browserHostId, bodyId, runBasis }) {
  const selectBrowser = async () => {
    await page.getByRole('button', { name: 'Inspect current wardrobe', exact: true }).click();
    await page.waitForFunction(() => !document.querySelector('[data-owner-wardrobe-refresh]').disabled);
    const current = await wardrobe();
    const route = current.route_descriptions.find(item => item.host_id === browserHostId);
    assert.ok(route, 'current browser route was not retained');
    const previous = current.route_descriptions.find(item => item.route_id === current.selected?.route_id);
    const change = async (verb, name) => {
      const button = page.getByRole('button', { name: `${verb} ${name}`, exact: true });
      if (!await button.isEnabled()) return;
      const revision = (await wardrobe()).wardrobe_revision_decimal;
      await button.click();
      await page.waitForFunction(prior => {
        const status = document.querySelector('[data-owner-wardrobe-status]');
        if (status?.dataset.refused === 'true') return true;
        try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent).wardrobe_revision_decimal !== prior
          && !document.querySelector('[data-owner-wardrobe-refresh]').disabled; }
        catch { return false; }
      }, revision, { timeout: 15_000 });
      assert.notEqual(await page.locator('[data-owner-wardrobe-status]').getAttribute('data-refused'), 'true',
        await page.locator('[data-owner-wardrobe-status]').textContent());
    };
    await change('Prefer only', route.mask_name);
    // Preference preserves a still-current Show; explicitly release the old
    // selection, then restore its eligibility for the next peer encounter.
    if (previous && previous.host_id !== browserHostId) {
      await change('Doff', previous.mask_name);
      await change('Wear', previous.mask_name);
    }
    assert.equal((await wardrobe()).selected?.route_id, route.route_id);
    await refresh();
    return wardrobe();
  };
  const screenshot = async name => captureOwnerViewport(page, path.join(output, name));
  // Retain a real active terminal Show before the browser changes its Face.
  await peer.command('show');
  const first = await peer.evidence();
  const terminalBefore = ownerFace();
  assert.equal(first.terminal.face_id, terminalBefore.face.identity);
  assert.equal(first.terminal.body_id, bodyId);
  await selectBrowser();
  await targetWording('21 °C');
  await action(raise(), 'journey-browser-raise', true);
  await action(raise(), 'journey-browser-raise-again');
  await action(raise(), 'journey-browser-target-22.5');
  await targetWording('22.5 °C');
  const beforeTerminal = await observe('journey-browser-before-terminal');
  await screenshot('thermostat-raised.png');
  await page.setViewportSize({ width: 390, height: 844 });
  await screenshot('thermostat-mobile.png');
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
  const beforeStale = ownerFace();
  await peer.command('action thermostat.mode.heat');
  const staleResponse = peer.actions().at(-1);
  assert.equal(staleResponse.accepted, false, 'the prior terminal Show must reach an owner refusal');
  const afterStale = ownerFace();
  assert.deepEqual(afterStale.face.properties, beforeStale.face.properties);
  await peer.command('action thermostat.mode.heat');
  assert.equal(peer.actions().at(-1).accepted, true);
  await peer.command('action thermostat.preset.eco');
  assert.equal(peer.actions().at(-1).accepted, true);
  const eco = ownerFace();
  const speechStarted = Date.now();
  await peer.command('read settings');
  const speechMillis = Date.now() - speechStarted;
  const final = await peer.evidence(true);
  assert.equal(final.terminal.face_id, eco.face.identity);
  assert.equal(final.spoken.execution.template.source_face_identity, eco.face.identity);
  assert.equal(captureDecimal(final.spoken.execution.template.source_face_revision), eco.face_revision_decimal);
  assert.equal(final.spoken.route.profile, 'hosted-presentation');
  assert.ok(!final.spoken.route.interaction_line);
  assert.equal(final.spoken.execution.template.template_identity, 'spoken/control@1');
  assert.equal(final.spoken.execution.source_show.show.host_id, peer.hostId);
  assert.ok(final.spoken.execution.frames.length > 1);
  const afterSpeech = ownerFace();
  assert.deepEqual(afterSpeech.face.properties, eco.face.properties);
  const actions = peer.actions();
  assert.deepEqual(actions.map(item => item.accepted), [false, true, true]);
  await peer.close();
  const artifacts = await retainSpokenArtifacts(final.spoken, peerState, output);
  const afterClose = ownerStatus();
  assert.equal(afterClose.biography.body_id, bodyId);
  assert.equal(afterClose.realization.plan.plan_id, runBasis.plan_id);
  assert.equal(afterClose.realization.play.active_play_id, runBasis.active_play_id);
  const finalWardrobe = await selectBrowser();
  await targetWording('18 °C');
  const afterTerminal = await observe('journey-browser-after-terminal-and-speech');
  const beforeBrowserStale = ownerFace();
  const browserStale = await page.evaluate(async view => {
    const choice = view.actions.find(item => item.name === 'Raise target by 0.5°C');
    try { return await globalThis.__thermostatProofSession.submitOwnerFaceInteraction({
      view, actionId: choice.identity, target: choice.target, arguments: [], sequence: 10000 }); }
    catch (error) { return { accepted: false, refusal: error.message }; }
  }, beforeTerminal.view);
  assert.equal(browserStale.accepted, false);
  const afterBrowserStale = ownerFace();
  assert.deepEqual(afterBrowserStale.face.properties, beforeBrowserStale.face.properties);
  await page.setViewportSize({ width: 1280, height: 1000 });
  await screenshot('thermostat-after-terminal.png');
  const bodyMenu = page.locator('conduit-owner-face details > summary').filter({ hasText: /^Body$/ });
  await bodyMenu.click();
  await bodyMenu.evaluate(element => element.scrollIntoView({ block: 'start', behavior: 'instant' }));
  await page.screenshot({ path: path.join(output, 'thermostat-body-menu.png'), animations: 'disabled' });
  await bodyMenu.click();
  owner(['body', 'lull', '--state-dir', state]);
  await refresh();
  const lulled = { owner: ownerFace(), status: ownerStatus(), view: await currentView() };
  assert.equal(lulled.status.realization, null);
  assert.ok(!lulled.owner.face.actions.some(item => item.identity.startsWith('thermostat.')));
  await screenshot('thermostat-lulled.png');
  owner(['body', 'start', '--state-dir', state, '--maximum-millis', '900000']);
  const deadline = Date.now() + 15_000;
  while (!ownerStatus().realization?.play?.active_play_id) {
    assert.ok(Date.now() < deadline);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  await refresh();
  await targetWording('21 °C');
  const restarted = { owner: ownerFace(), status: ownerStatus(), view: await currentView() };
  assert.equal(restarted.status.biography.body_id, bodyId);
  assert.notEqual(restarted.status.realization.play.active_play_id, runBasis.active_play_id);
  await screenshot('thermostat-restarted.png');
  owner(['body', 'lull', '--state-dir', state]);
  return { before_terminal: beforeTerminal, terminal_evidence: [first.terminal, final.terminal],
    terminal_before: terminalBefore, terminal_stale: { ...staleResponse, before: beforeStale, after: afterStale },
    terminal_actions: actions, after_terminal: afterTerminal, after_peer_closed: afterClose,
    speech: { accepted: true, elapsed_millis: speechMillis, evidence: final.spoken,
      speaker_delivery: false, human_listening: false, before: eco, after: afterSpeech, artifacts }, lulled, restarted,
    browser_stale: { ...browserStale, before: beforeBrowserStale, after: afterBrowserStale }, final_wardrobe: finalWardrobe };
}
