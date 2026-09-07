import { expect, test } from "@playwright/test";
import { mkdir, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import { startWebRtcRendezvousProbe } from "./browser-presence-support.mjs";

test("two admitted product clients compose Body grants into one exact ready session", async ({
  browser,
}, testInfo) => {
  const probe = await startWebRtcRendezvousProbe();
  const sourceContext = await browser.newContext();
  const sinkContext = await browser.newContext();
  const source = await sourceContext.newPage();
  const sink = await sinkContext.newPage();
  const browserErrors = [];
  for (const page of [source, sink]) {
    page.on("pageerror", (error) => browserErrors.push(error.message));
  }
  await Promise.all([
    source.goto(`/proof/browser/browser-presence.test.html?renew=false&reconnect=false&body=${encodeURIComponent(probe.url)}`),
    sink.goto(`/proof/browser/browser-presence.test.html?renew=false&reconnect=false&body=${encodeURIComponent(probe.url)}`),
  ]);
  for (const page of [source, sink]) {
    await expect.poll(
      () => page.evaluate(() => globalThis.__browserPresence?.presenceState()),
      {
        message: `browser entrance did not establish presence; browser errors=${browserErrors.join(" | ")} probe=${probe.output()}`,
        timeout: 15_000,
      },
    )
      .toBe("available");
    await expect.poll(
      () => page.evaluate(() => globalThis.__browserPresence.webRtcSessions()),
      {
        message: `granted WebRTC session did not finish creation; browser errors=${browserErrors.join(" | ")} probe=${probe.output()}`,
        timeout: 15_000,
      },
    )
      .toMatchObject({ activeSessions: 1, creatingSessions: 0, pendingSignals: 0, failure: null });
  }
  await expect.poll(async () => JSON.stringify(await Promise.all([source, sink].map(
    (page) => page.evaluate(() => globalThis.__browserPresence.webRtcSessions()),
  ))), {
    message: `browser errors=${browserErrors.join(" | ")} probe=${probe.output()}`,
    timeout: 15_000,
  }).toContain('"sessionReady":true');
  for (const page of [source, sink]) {
    await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.webRtcSessions().sessions[0]))
      .toMatchObject({ sessionReady: true, terminalReason: null, terminalDetail: null });
  }
  const sourceIdentity = await source.evaluate(() => ({
    hostId: globalThis.__browserPresence.hostId,
    bootId: globalThis.__browserPresence.bootId,
  }));
  const sinkIdentity = await sink.evaluate(() => ({
    hostId: globalThis.__browserPresence.hostId,
    bootId: globalThis.__browserPresence.bootId,
  }));
  const sourceSession = await source.evaluate(() => globalThis.__browserPresence.webRtcSessions().sessions[0]);
  const sinkSession = await sink.evaluate(() => globalThis.__browserPresence.webRtcSessions().sessions[0]);
  expect([sourceSession.role, sinkSession.role].sort()).toEqual(["sink", "source"]);
  expect(sourceSession).toMatchObject({ peerHostId: sinkIdentity.hostId, peerBootId: sinkIdentity.bootId });
  expect(sinkSession).toMatchObject({ peerHostId: sourceIdentity.hostId, peerBootId: sourceIdentity.bootId });
  const sourcePage = sourceSession.role === "source" ? source : sink;
  const sinkPage = sourceSession.role === "sink" ? source : sink;
  const sourceRoleIdentity = sourceSession.role === "source" ? sourceIdentity : sinkIdentity;
  const sinkRoleIdentity = sourceSession.role === "sink" ? sourceIdentity : sinkIdentity;
  const negotiationId = sourceSession.negotiationId;
  const value = [11, 22, 33, 44];
  await expect(sourcePage.evaluate(
    (id) => globalThis.__browserPresence.receiveWebRtcValue(id),
    negotiationId,
  )).rejects.toThrow("WebRTC sink role required");
  await expect(sourcePage.evaluate(
    (id) => globalThis.__browserPresence.offerWebRtcValue(`${id}/stale`, [1]),
    negotiationId,
  )).rejects.toThrow("unknown or stale WebRTC negotiation identity");

  await sinkPage.evaluate((id) => globalThis.__browserPresence.pressureNextWebRtcValue(id), negotiationId);
  const pressured = await sourcePage.evaluate(
    ({ id, bytes }) => globalThis.__browserPresence.offerWebRtcValue(id, bytes),
    { id: negotiationId, bytes: value },
  );
  expect(pressured).toEqual({
    accepted: false,
    retryable: true,
    reason: "peer-pressure",
    sequence: 0,
    delivered: false,
  });
  await expect(sourcePage.evaluate(
    ({ id, bytes }) => globalThis.__browserPresence.offerWebRtcValue(id, bytes),
    { id: negotiationId, bytes: new Array(17).fill(1) },
  )).rejects.toThrow("Cord offer refused -215");

  const receive = sinkPage.evaluate(
    (id) => globalThis.__browserPresence.receiveWebRtcValue(id),
    negotiationId,
  );
  await expect(sinkPage.evaluate(
    (id) => globalThis.__browserPresence.receiveWebRtcValue(id),
    negotiationId,
  )).rejects.toThrow("Cord receive already pending");
  const accepted = sourcePage.evaluate(
    ({ id, bytes }) => globalThis.__browserPresence.offerWebRtcValue(id, bytes),
    { id: negotiationId, bytes: value },
  );
  const acceptedOutcome = await accepted;
  expect(acceptedOutcome).toEqual({
    accepted: true,
    retryable: false,
    reason: null,
    sequence: 0,
    delivered: false,
  });
  const received = await receive;
  expect(received).toEqual({ sequence: 0, bytes: value });
  await expect(sinkPage.evaluate(
    (id) => globalThis.__browserPresence.receiveWebRtcValue(id),
    negotiationId,
  )).rejects.toThrow("Cord value already received");
  await expect(sourcePage.evaluate(
    ({ id, bytes }) => globalThis.__browserPresence.offerWebRtcValue(id, bytes),
    { id: negotiationId, bytes: [55] },
  )).rejects.toThrow("Cord value already in flight");
  const delivered = sourcePage.evaluate(
    ({ id, sequence }) => globalThis.__browserPresence.waitWebRtcValueDelivered(id, sequence),
    { id: negotiationId, sequence: received.sequence },
  );
  expect(await sinkPage.evaluate(
    ({ id, sequence }) => globalThis.__browserPresence.deliverWebRtcValue(id, sequence),
    { id: negotiationId, sequence: received.sequence },
  )).toEqual({ delivered: true, sequence: 0 });
  expect(await delivered).toEqual({ delivered: true, sequence: 0 });
  await expect(sinkPage.evaluate(
    ({ id, sequence }) => globalThis.__browserPresence.deliverWebRtcValue(id, sequence),
    { id: negotiationId, sequence: received.sequence },
  )).rejects.toThrow("Cord delivery identity refused");
  for (const page of [sourcePage, sinkPage]) {
    await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.webRtcSessions().sessions[0]))
      .toMatchObject({ acceptedSequence: 0, deliveredSequence: 0, valuePending: false });
  }
  const pendingReceive = expect(sinkPage.evaluate(
    (id) => globalThis.__browserPresence.receiveWebRtcValue(id),
    negotiationId,
  )).rejects.toThrow(/closed|traffic|Line/);
  await expect.poll(() => sinkPage.evaluate(
    () => globalThis.__browserPresence.webRtcSessions().sessions[0].receivePending,
  )).toBe(true);
  await sourcePage.evaluate(
    (id) => globalThis.__browserPresence.closeWebRtcLine(id),
    negotiationId,
  );
  await pendingReceive;
  for (const page of [sourcePage, sinkPage]) {
    await expect.poll(() => page.evaluate(() => {
      const session = globalThis.__browserPresence.webRtcSessions().sessions[0];
      return session !== undefined
        && !session.sessionReady
        && session.terminalReason === "line-closed"
        && session.line.readyState === "closed";
    })).toBe(true);
    await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.presenceState()))
      .toBe("available");
  }
  const sourceAfterLoss = await sourcePage.evaluate(() => globalThis.__browserPresence.webRtcSessions().sessions[0]);
  const sinkAfterLoss = await sinkPage.evaluate(() => globalThis.__browserPresence.webRtcSessions().sessions[0]);
  const sourcePresenceAfterLoss = await sourcePage.evaluate(() => ({
    hostId: globalThis.__browserPresence.hostId,
    bootId: globalThis.__browserPresence.bootId,
    state: globalThis.__browserPresence.presenceState(),
  }));
  const sinkPresenceAfterLoss = await sinkPage.evaluate(() => ({
    hostId: globalThis.__browserPresence.hostId,
    bootId: globalThis.__browserPresence.bootId,
    state: globalThis.__browserPresence.presenceState(),
  }));
  await expect(sourcePage.evaluate(
    (id) => globalThis.__browserPresence.offerWebRtcValue(id, [1]),
    negotiationId,
  )).rejects.toThrow("WebRTC session is not current and Ready");
  await expect.poll(probe.output).toContain("relayed stage=2");
  expect(await Promise.all([source, sink].map(
    (page) => page.evaluate(() => globalThis.__browserPresence.replanWebRtc()),
  ))).toEqual([1, 1]);
  for (const page of [source, sink]) {
    await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.webRtcSessions()))
      .toMatchObject({
        generation: 1,
        activeSessions: 1,
        creatingSessions: 0,
        pendingSignals: 0,
        retiredNegotiations: 1,
        failure: null,
      });
    await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.webRtcSessions().sessions[0]))
      .toMatchObject({ sessionReady: true, terminalReason: null });
  }
  await expect.poll(probe.output).toContain("relayed stage=4");
  await expect.poll(probe.output).toContain("stale-grant-callback generation=0 current=1");
  await expect.poll(async () => (await Promise.all([source, sink].map(
    (page) => page.evaluate(() => globalThis.__browserPresence.webRtcSessions().refusal),
  ))).filter((refusal) => refusal === "stale WebRTC grant generation").length).toBe(1);
  const replacementSourceSession = await source.evaluate(
    () => globalThis.__browserPresence.webRtcSessions().sessions[0],
  );
  const replacementSinkSession = await sink.evaluate(
    () => globalThis.__browserPresence.webRtcSessions().sessions[0],
  );
  expect([replacementSourceSession.role, replacementSinkSession.role].sort()).toEqual(["sink", "source"]);
  const replacementSourcePage = replacementSourceSession.role === "source" ? source : sink;
  const replacementSinkPage = replacementSourceSession.role === "sink" ? source : sink;
  const replacementNegotiationId = replacementSourceSession.negotiationId;
  expect(replacementNegotiationId).not.toBe(negotiationId);
  await expect(replacementSourcePage.evaluate(
    (id) => globalThis.__browserPresence.offerWebRtcValue(id, [9]),
    negotiationId,
  )).rejects.toThrow("unknown or stale WebRTC negotiation identity");
  const replacementValue = [55, 66, 77];
  const replacementReceive = replacementSinkPage.evaluate(
    (id) => globalThis.__browserPresence.receiveWebRtcValue(id),
    replacementNegotiationId,
  );
  const replacementAccepted = await replacementSourcePage.evaluate(
    ({ id, bytes }) => globalThis.__browserPresence.offerWebRtcValue(id, bytes),
    { id: replacementNegotiationId, bytes: replacementValue },
  );
  expect(replacementAccepted).toEqual({
    accepted: true,
    retryable: false,
    reason: null,
    sequence: 0,
    delivered: false,
  });
  const replacementReceived = await replacementReceive;
  expect(replacementReceived).toEqual({ sequence: 0, bytes: replacementValue });
  const replacementDelivered = replacementSourcePage.evaluate(
    ({ id, sequence }) => globalThis.__browserPresence.waitWebRtcValueDelivered(id, sequence),
    { id: replacementNegotiationId, sequence: 0 },
  );
  expect(await replacementSinkPage.evaluate(
    ({ id, sequence }) => globalThis.__browserPresence.deliverWebRtcValue(id, sequence),
    { id: replacementNegotiationId, sequence: 0 },
  )).toEqual({ delivered: true, sequence: 0 });
  expect(await replacementDelivered).toEqual({ delivered: true, sequence: 0 });
  const replacementStates = await Promise.all([source, sink].map(
    (page) => page.evaluate(() => globalThis.__browserPresence.webRtcSessions()),
  ));
  // Close the selected Line explicitly, then lose the presence transport without
  // sending the distinct acknowledged PresenceLeave protocol. These are separate
  // observations; do not rely on an unbounded ICE failure-detection interval.
  await sourcePage.evaluate(
    (id) => globalThis.__browserPresence.closeWebRtcLine(id),
    replacementNegotiationId,
  );
  await sourcePage.close();
  await expect.poll(probe.output).toContain("peer-lost");
  await expect.poll(() => probe.output().match(/^host_loss=(\{.*\})$/m)?.[1]).toBeDefined();
  const hostLossMatch = probe.output().match(/^host_loss=(\{.*\})$/m);
  expect(hostLossMatch, probe.output()).not.toBeNull();
  const hostLoss = JSON.parse(hostLossMatch[1]);
  await expect.poll(() => sinkPage.evaluate(
    () => globalThis.__browserPresence.webRtcSessions().sessions[0]?.terminalReason,
  )).toBe("line-closed");
  await expect.poll(() => sinkPage.evaluate(() => globalThis.__browserPresence.presenceState()))
    .toBe("available");
  const restartedContext = await browser.newContext();
  const restartedPage = await restartedContext.newPage();
  await restartedPage.goto(
    `/proof/browser/browser-presence.test.html?renew=false&reconnect=false&body=${encodeURIComponent(probe.url)}`,
  );
  await expect.poll(async () => {
    if (probe.process.exitCode !== null) throw new Error(probe.output());
    return restartedPage.evaluate(() => globalThis.__browserPresence?.presenceState());
  }).toBe("available");
  const restartedIdentity = await restartedPage.evaluate(() => ({
    hostId: globalThis.__browserPresence.hostId,
    bootId: globalThis.__browserPresence.bootId,
    state: globalThis.__browserPresence.state(),
  }));
  await expect.poll(() => probe.output().match(/^restart=(\{.*\})$/m)?.[1]).toBeDefined();
  const restartMatch = probe.output().match(/^restart=(\{.*\})$/m);
  expect(restartMatch, probe.output()).not.toBeNull();
  const restart = JSON.parse(restartMatch[1]);
  // This probe observes transport loss, not acknowledged voluntary departure.
  await restartedContext.close();
  await sourceContext.close();
  await sinkContext.close();
  await expect.poll(() => {
    if (probe.process.exitCode && probe.process.exitCode !== 0) throw new Error(probe.output());
    return probe.process.exitCode;
  }).toBe(0);
  const basisMatch = probe.output().match(/^session_basis=(\{.*\})$/m);
  expect(basisMatch, probe.output()).not.toBeNull();
  const basis = JSON.parse(basisMatch[1]);
  const replacementBasisMatch = probe.output().match(/^replacement_basis=(\{.*\})$/m);
  expect(replacementBasisMatch, probe.output()).not.toBeNull();
  const replacementBasis = JSON.parse(replacementBasisMatch[1]);
  expect(basis).toMatchObject({
    base: "conduit.base/webrtc-data-channel@1",
    session_limits: { maximum_in_flight_items: 1, maximum_payload_bytes: 16, maximum_buffered_bytes: 16 },
    line_limits: { maximum_in_flight_items: 1, maximum_payload_bytes: 16, maximum_buffered_bytes: 16 },
  });
  const evidenceRoot = process.env.CONDUIT_EVIDENCE_ROOT;
  if (["chromium", "firefox"].includes(testInfo.project.name) && evidenceRoot) {
    const receipt = {
      schema: "conduit.browser-host/body-granted-webrtc-session@1",
      proof_class: "live-browser",
      browser_engine: testInfo.project.name,
      browser_version: browser.version(),
      basis,
      browser_peers: [
        { ...sourceIdentity, session: sourceSession },
        { ...sinkIdentity, session: sinkSession },
      ],
      traffic: {
        value,
        pressured,
        accepted: acceptedOutcome,
        received,
        delivered: { delivered: true, sequence: received.sequence },
      },
      loss: {
        source_presence: sourcePresenceAfterLoss,
        sink_presence: sinkPresenceAfterLoss,
        source_session: sourceAfterLoss,
        sink_session: sinkAfterLoss,
        line_ready_before: true,
        line_ready_after: false,
      },
      replacement: {
        basis: replacementBasis,
        browser_peers: [
          { ...sourceIdentity, session: replacementSourceSession },
          { ...sinkIdentity, session: replacementSinkSession },
        ],
        traffic: {
          value: replacementValue,
          accepted: replacementAccepted,
          received: replacementReceived,
          delivered: { delivered: true, sequence: replacementReceived.sequence },
        },
        session_states: replacementStates,
      },
      host_loss: hostLoss,
      restart,
      assertions: {
        distinct_host_boot_peers: sourceIdentity.hostId !== sinkIdentity.hostId
          && sourceIdentity.bootId !== sinkIdentity.bootId,
        body_grant_session_exact: sourceSession.negotiationId === basis.binding_id
          && sinkSession.negotiationId === basis.binding_id
          && sourceSession.peerHostId === sinkIdentity.hostId
          && sourceSession.peerBootId === sinkIdentity.bootId
          && sinkSession.peerHostId === sourceIdentity.hostId
          && sinkSession.peerBootId === sourceIdentity.bootId,
        ordinary_cord_value_delivered: received.sequence === 0
          && received.bytes.join(",") === value.join(",")
          && acceptedOutcome.accepted
          && sinkAfterLoss.deliveredSequence === received.sequence,
        membership_presence_distinct_from_line_readiness:
          sourcePresenceAfterLoss.state === "available"
          && sinkPresenceAfterLoss.state === "available"
          && sourcePresenceAfterLoss.hostId === sourceRoleIdentity.hostId
          && sourcePresenceAfterLoss.bootId === sourceRoleIdentity.bootId
          && sinkPresenceAfterLoss.hostId === sinkRoleIdentity.hostId
          && sinkPresenceAfterLoss.bootId === sinkRoleIdentity.bootId
          && sourceAfterLoss.terminalReason === "line-closed"
          && sinkAfterLoss.terminalReason === "line-closed"
          && sourceAfterLoss.line.readyState === "closed"
          && sinkAfterLoss.line.readyState === "closed",
        explicit_replan_is_distinct_and_old_truth_is_immutable:
          basis.generation === 0
          && replacementBasis.generation === 1
          && basis.body_id === replacementBasis.body_id
          && basis.source_part_id === replacementBasis.source_part_id
          && basis.sink_part_id === replacementBasis.sink_part_id
          && basis.connection_id === replacementBasis.connection_id
          && basis.plan_id !== replacementBasis.plan_id
          && basis.source_active_play_id !== replacementBasis.source_active_play_id
          && basis.sink_active_play_id !== replacementBasis.sink_active_play_id
          && basis.line_id !== replacementBasis.line_id
          && basis.binding_id !== replacementBasis.binding_id
          && basis.base_instance_id !== replacementBasis.base_instance_id
          && replacementNegotiationId === replacementBasis.binding_id
          && replacementReceived.bytes.join(",") === replacementValue.join(",")
          && replacementStates.every((state) => state.generation === 1
            && state.retiredNegotiations === 1
            && state.activeSessions === 1
            && state.failure === null)
          && replacementStates.filter(
            (state) => state.refusal === "stale WebRTC grant generation",
          ).length === 1,
        host_loss_retains_part_but_invalidates_current_boot_and_lines:
          hostLoss.lost_part.state === "Admitted"
          && hostLoss.lost_part.current === null
          && hostLoss.lost_presence.state === "Unavailable"
          && hostLoss.lost_presence.host_id === sourceRoleIdentity.hostId
          && hostLoss.lost_presence.boot_id === sourceRoleIdentity.bootId
          && hostLoss.survivor_part.state === "Admitted"
          && hostLoss.survivor_part.current.host_id === sinkRoleIdentity.hostId
          && hostLoss.survivor_part.current.boot_id === sinkRoleIdentity.bootId
          && hostLoss.survivor_presence.state === "Available"
          && hostLoss.invalidated_binding_ids.includes(replacementBasis.binding_id)
          && hostLoss.lost_grant_total === 0
          && !hostLoss.lost_grant_present
          && hostLoss.survivor_grant_total === 0
          && !hostLoss.survivor_grant_present,
        restart_creates_fresh_truth_without_rebinding_the_offline_part:
          restart.old_part.part_id === hostLoss.lost_part.part_id
          && restart.old_part.state === "Admitted"
          && restart.old_part.current === null
          && restart.old_presence.state === "Unavailable"
          && restart.old_host_id === hostLoss.lost_presence.host_id
          && restart.old_boot_id === hostLoss.lost_presence.boot_id
          && restart.fresh_part.part_id === restart.fresh_credential.part_id
          && restart.fresh_part.part_id !== restart.old_part.part_id
          && restart.fresh_part.current.host_id === restartedIdentity.hostId
          && restart.fresh_part.current.boot_id === restartedIdentity.bootId
          && restart.fresh_presence.state === "Available"
          && restart.fresh_credential.host_id === restartedIdentity.hostId
          && restart.fresh_credential.boot_id === restartedIdentity.bootId
          && restartedIdentity.hostId !== restart.old_host_id
          && restartedIdentity.bootId !== restart.old_boot_id
          && restart.old_grant_total === 0
          && !restart.old_grant_present
          && restart.fresh_grant_total === 0
          && !restart.fresh_grant_present
          && restart.membership_part_count === 3
          && restart.presence_lease_count === 3,
        state_is_finite: basis.session_limits.maximum_in_flight_items === 1
          && basis.session_limits.maximum_payload_bytes === 16
          && basis.session_limits.maximum_buffered_bytes === 16
          && sourceAfterLoss.line.bufferedBytes === 0
          && sourceAfterLoss.line.retainedMessages === 0
          && sourceAfterLoss.line.retainedBytes === 0
          && sinkAfterLoss.line.bufferedBytes === 0
          && sinkAfterLoss.line.retainedMessages === 0
          && sinkAfterLoss.line.retainedBytes === 0,
      },
    };
    await mkdir(evidenceRoot, { recursive: true });
    const destination = path.join(
      evidenceRoot,
      testInfo.project.name === "chromium"
        ? "browser-webrtc-session.json"
        : `browser-webrtc-session-${testInfo.project.name}.json`,
    );
    const temporary = `${destination}.tmp`;
    await writeFile(temporary, `${JSON.stringify(receipt, null, 2)}\n`);
    await rename(temporary, destination);
  }
});
