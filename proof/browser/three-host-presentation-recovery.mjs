// Exercise loss of the browser presentation route while the Linux owner and
// QMP guest remain admitted. A new browser Boot must obtain a new owner Show.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

export async function capturePresentationRecovery({ page, context, serverUrl, owner, state,
  bodyId, ownerPartId, guestPartId, browserCredential, browserBootId, oldFace,
  oldWardrobe, output, sourceCommit, runId }) {
  const status = () => {
    const result = owner(['body', 'status', '--state-dir', state, '--json']);
    assert.equal(result.biography.body_id, bodyId);
    return result;
  };
  assert.equal(oldWardrobe.selected?.route_id,
    oldWardrobe.route_descriptions.find(route => route.host_id === browserCredential.host_id)?.route_id);
  assert.equal(oldFace.show_id, oldWardrobe.show_id);
  await page.getByRole('button', { name: 'Leave this window' }).click();
  try {
    await page.waitForFunction(() => {
      const participation = globalThis.__conduitOwnerParticipation;
      return participation?.presence() !== 'available' && participation?.face() === null
        && [...document.querySelectorAll('[data-owner-action] button')]
          .every(button => button.disabled)
        && document.querySelector('[data-owner-wardrobe-evidence]')?.textContent
          .includes('historical')
        && document.querySelector('[data-owner-action-result]')?.textContent
          .includes('no current action return');
    }, null, { timeout: 12_000 });
  } catch (error) {
    const state = await page.evaluate(() => ({
      presence: globalThis.__conduitOwnerParticipation?.presence(),
      faceCurrent: globalThis.__conduitOwnerParticipation?.face() !== null,
      actionButtons: [...document.querySelectorAll('[data-owner-action] button')]
        .map(button => ({ disabled: button.disabled, text: button.textContent })),
      wardrobeHistorical: document.querySelector('[data-owner-wardrobe-evidence]')?.textContent
        .includes('historical'),
      actionResult: document.querySelector('[data-owner-action-result]')?.textContent,
      status: document.querySelector('[data-owner-status]')?.textContent,
    }));
    throw new Error(`browser loss did not settle: ${JSON.stringify(state)}`, { cause: error });
  }
  const lost = await page.evaluate(() => ({
    presence: globalThis.__conduitOwnerParticipation.presence(),
    face: globalThis.__conduitOwnerParticipation.face(),
    actionDisabled: [...document.querySelectorAll('[data-owner-action] button')]
      .every(button => button.disabled),
    result: document.querySelector('[data-owner-action-result]').textContent,
    wardrobe: document.querySelector('[data-owner-wardrobe-evidence]').textContent,
  }));
  assert.notEqual(lost.presence, 'available');
  assert.equal(lost.face, null);
  assert.equal(lost.actionDisabled, true);
  assert.match(lost.result, /no current action return/);
  assert.match(lost.wardrobe, /historical/);
  const duringLoss = status();
  for (const partId of [ownerPartId, guestPartId]) {
    assert.ok(duringLoss.biography.membership.parts.find(part => part.part_id === partId)?.current);
  }
  await page.close();

  const recoveredPage = await context.newPage();
  await recoveredPage.goto(`${serverUrl}?participate=owner#your-handbook`);
  await recoveredPage.locator('[data-owner-key]').waitFor();
  const identity = await recoveredPage.evaluate(() =>
    globalThis.__conduitOwnerParticipation.admissionIdentity());
  assert.equal(identity.hostId, browserCredential.host_id);
  assert.notEqual(identity.bootId, browserBootId, 'recovery needs a fresh browser Boot');
  const window = owner(['body', 'browser-window', '--state-dir', state,
    '--expected-host-id', identity.hostId,
    '--new-host-verifying-key', JSON.stringify(identity.verifyingKey),
    '--maximum-millis', '60000', '--authorize-window']);
  assert.equal(window.body_id, bodyId);
  await recoveredPage.getByLabel('Body ID').fill(bodyId);
  await recoveredPage.getByLabel('Owner window URL').fill(window.url);
  await recoveredPage.getByRole('button', { name: 'Join this Body' }).click();
  await recoveredPage.waitForFunction(() => globalThis.__conduitOwnerParticipation?.presence() === 'available',
    null, { timeout: 12_000 });
  await recoveredPage.locator('[data-owner-face-document] [data-owner-action]').first().waitFor();
  const recovered = await recoveredPage.evaluate(() => ({
    credential: globalThis.__conduitOwnerParticipation.credential(),
    face: globalThis.__conduitOwnerParticipation.face(),
  }));
  assert.equal(recovered.credential.body_id, bodyId);
  assert.equal(recovered.credential.part_id, browserCredential.part_id,
    'recovery must use the retained Part, not birth another Body or Part');
  assert.equal(recovered.face.body_id, bodyId);
  assert.notEqual(recovered.face.show_id, oldFace.show_id);
  assert.equal(await recoveredPage.locator('[data-handbook-application]')
    .getAttribute('data-owner-show-acknowledged'), recovered.face.show_id);
  await recoveredPage.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  await recoveredPage.waitForFunction(() => {
    try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
      .schema === 'conduit.body/owner-mask-wardrobe@1'; } catch { return false; }
  }, null, { timeout: 12_000 });
  const wardrobe = JSON.parse(await recoveredPage.locator('[data-owner-wardrobe-evidence]').textContent());
  assert.equal(wardrobe.body_id, bodyId);
  assert.equal(wardrobe.show_id, recovered.face.show_id);
  assert.equal(wardrobe.fresh_show_required, false);
  assert.equal(wardrobe.selected?.route_id,
    wardrobe.route_descriptions.find(route => route.host_id === identity.hostId)?.route_id);
  const after = status();
  assert.equal(after.biography.membership.parts.length, duringLoss.biography.membership.parts.length);
  assert.equal(after.biography.membership.parts.find(part =>
    part.part_id === browserCredential.part_id)?.current?.boot_id, identity.bootId);
  assert.equal(after.biography.membership.parts.find(part =>
    part.part_id === ownerPartId)?.current?.boot_id,
    duringLoss.biography.membership.parts.find(part => part.part_id === ownerPartId)?.current?.boot_id);
  await recoveredPage.locator('[data-owner-face]').screenshot({
    path: path.join(output, 'browser-after-recovery.png'),
  });
  const receipt = {
    schema: 'conduit.proof/browser-presentation-recovery@1', source_commit: sourceCommit,
    run_id: runId, body_id: bodyId, owner_part_id: ownerPartId, guest_part_id: guestPartId,
    browser_part_id: browserCredential.part_id,
    lost_browser_boot_id: browserBootId, recovered_browser_boot_id: identity.bootId,
    old_show_id: oldFace.show_id, recovered_show_id: recovered.face.show_id,
    old_owner_plan_id: oldWardrobe.owner_plan_id, recovered_owner_plan_id: wardrobe.owner_plan_id,
    loss: lost, membership_during_loss: duringLoss.biography.membership,
    recovered_wardrobe: wardrobe, membership_after_recovery: after.biography.membership,
    proof_boundary: 'Browser window leave and new Boot admission; owner and QMP Parts stay current. No QMP reboot or workload failover claimed.',
  };
  const bytes = Buffer.from(`${JSON.stringify(receipt, null, 2)}\n`);
  await writeFile(path.join(output, 'browser-presentation-recovery.json'), bytes);
  return { receipt, bytes, screenshot: await readFile(path.join(output, 'browser-after-recovery.png')) };
}
