import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";
import { mkdir, rename, writeFile } from "node:fs/promises";
import path from "node:path";

async function startPresenceProbe(extraArguments = []) {
  const process = spawn("target/debug/browser-admission-probe", ["--presence", ...extraArguments], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`presence probe was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/^(ws:\/\/[^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    process.stdout.on("data", inspect);
    process.stderr.on("data", inspect);
    process.once("exit", (code) => {
      clearTimeout(timeout);
      if (code !== 0) reject(new Error(`presence probe exited (${code})\n${output}`));
    });
  });
  return { process, url, output: () => output };
}

async function startWebRtcRendezvousProbe() {
  const process = spawn("target/debug/browser-webrtc-rendezvous-probe", [], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`rendezvous probe was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/^(ws:\/\/[^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    process.stdout.on("data", inspect);
    process.stderr.on("data", inspect);
    process.once("exit", (code) => {
      clearTimeout(timeout);
      if (code !== 0) reject(new Error(`rendezvous probe exited (${code})\n${output}`));
    });
  });
  return { process, url, output: () => output };
}

test("admitted browser renews exact current presence and close makes it unavailable", async ({ page }) => {
  const probe = await startPresenceProbe();
  await page.goto(`/hosts/browser/browser-presence.test.html?body=${encodeURIComponent(probe.url)}`);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence?.presenceState())).toBe("available");
  await expect.poll(probe.output).toContain("renewed sequence=2");
  await expect.poll(probe.output).toContain("renewed sequence=3");
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.freshnessProfile().sequence)).toBeGreaterThanOrEqual(3);
  expect(await page.evaluate(() => globalThis.__browserPresence.freshnessProfile())).toMatchObject({
    scheduling: "best-effort-browser-event-loop",
    availabilityAuthority: "server-session-or-lease",
    backgroundRealtimeGuarantee: false,
    maximumReconnectAttempts: 1,
    renewAfterMillis: 500,
  });
  expect(["visible", "hidden"]).toContain(
    await page.evaluate(() => globalThis.__browserPresence.pageLifecycle()),
  );
  await expect(page.evaluate(() => globalThis.__browserPresence.signalWebRtc({
    targetHostId: "",
    targetBootId: "browser-boot/absent",
    signal: {},
  }))).rejects.toThrow("invalid WebRTC signaling target or payload");
  await expect(page.evaluate(() => globalThis.__browserPresence.requestWebRtcGrant(16)))
    .rejects.toThrow("invalid WebRTC grant generation or index");
  await page.evaluate(() => globalThis.__browserPresence.requestWebRtcGrant(0));
  await expect.poll(() => page.evaluate(() => globalThis.__browserWebRtcGrants.at(-1)))
    .toEqual({ kind: "web-rtc-grant", protocol: 1, generation: 0, index: 0, total: 0, grant: null });
  expect(await page.evaluate(() => Object.isFrozen(globalThis.__browserWebRtcGrants.at(-1))))
    .toBe(true);
  expect(await page.evaluate(() => {
    const source = {
      kind: "web-rtc-grant",
      protocol: 1,
      generation: 0,
      index: 0,
      total: 1,
      grant: {
        negotiation_id: "binding/exact",
        role: "source",
        peer_host_id: "host/peer",
        peer_boot_id: "boot/peer",
        session_hello: [1, 2, 3],
      },
    };
    const immutable = globalThis.__immutableWebRtcGrantFrame(source);
    source.grant.peer_host_id = "host/mutated";
    source.grant.session_hello[0] = 9;
    return {
      outer: Object.isFrozen(immutable),
      grant: Object.isFrozen(immutable.grant),
      hello: Object.isFrozen(immutable.grant.session_hello),
      peerHostId: immutable.grant.peer_host_id,
      firstByte: immutable.grant.session_hello[0],
    };
  })).toEqual({
    outer: true,
    grant: true,
    hello: true,
    peerHostId: "host/peer",
    firstByte: 1,
  });
  await expect(page.evaluate(() => globalThis.__immutableWebRtcSignalFrame({
    signal: { negotiation_id: "binding/malformed" },
  }))).rejects.toThrow("invalid WebRTC signal frame");
  expect(await page.evaluate(() => globalThis.__probeConcurrentGrantStage())).toMatchObject({
    creations: 1,
    requests: [[0, 0]],
    secondRefusal: "WebRTC grant creation already in flight",
    creationRefusal: "creation refused",
    state: {
      nextGrantIndex: 1,
      inFlightGrantIndex: null,
      activeSessions: 1,
      creatingSessions: 0,
    },
    failedState: {
      nextGrantIndex: 0,
      inFlightGrantIndex: null,
      activeSessions: 0,
      creatingSessions: 0,
    },
    staleRefusal: "stale WebRTC session creation",
    ownershipCloses: 2,
    closesAfterReset: 1,
    resetState: {
      inFlightGrantIndex: null,
      activeSessions: 0,
      creatingSessions: 0,
      terminalReason: "test reset",
    },
    stateAfterStale: {
      nextGrantIndex: 0,
      inFlightGrantIndex: 0,
      activeSessions: 0,
      creatingSessions: 1,
    },
    generationState: {
      nextGrantIndex: 1,
      inFlightGrantIndex: null,
      activeSessions: 1,
      creatingSessions: 0,
    },
  });
  expect(await page.evaluate(() => {
    const source = {
      kind: "web-rtc-signal",
      protocol: 1,
      source_host_id: "host/peer",
      source_boot_id: "boot/peer",
      signal: {
        negotiation_id: "binding/exact",
        description: "offer",
        session_hello: [1, 2, 3],
        sdp: "bounded",
      },
    };
    const immutable = globalThis.__immutableWebRtcSignalFrame(source);
    source.signal.description = "answer";
    source.signal.session_hello[0] = 9;
    return {
      outer: Object.isFrozen(immutable),
      signal: Object.isFrozen(immutable.signal),
      hello: Object.isFrozen(immutable.signal.session_hello),
      description: immutable.signal.description,
      firstByte: immutable.signal.session_hello[0],
    };
  })).toEqual({
    outer: true,
    signal: true,
    hello: true,
    description: "offer",
    firstByte: 1,
  });
  await expect.poll(probe.output).toContain("webrtc-grant generation=0 index=0 total=0");
  const finalSequence = await page.evaluate(() => globalThis.__browserPresence.close());
  expect(finalSequence).toBeGreaterThanOrEqual(3);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.state())).toBe("offline");
  await expect.poll(probe.output).toContain(`unavailable reason=session-lost sequence=${finalSequence}`);
  await expect.poll(() => probe.process.exitCode).toBe(0);
  const acceptedRenewals = [...probe.output().matchAll(/renewed sequence=(\d+)/g)]
    .map(([, sequence]) => Number(sequence));
  expect(Math.max(...acceptedRenewals)).toBe(finalSequence);
});

test("admitted browser that stops renewing becomes unavailable only at lease expiry", async ({ page }) => {
  const probe = await startPresenceProbe();
  await page.goto(`/hosts/browser/browser-presence.test.html?renew=false&reconnect=false&body=${encodeURIComponent(probe.url)}`);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence?.presenceState())).toBe("available");
  await expect.poll(probe.output).toContain("unavailable reason=expired sequence=1");
  await expect.poll(() => probe.process.exitCode).toBe(0);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.state())).toBe("offline");
});

test("same running browser returns after session loss with exact Host and Boot", async ({ page }) => {
  const probe = await startPresenceProbe(["--reconnect"]);
  await page.goto(`/hosts/browser/browser-presence.test.html?body=${encodeURIComponent(probe.url)}`);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence?.presenceState())).toBe("available");
  const identity = await page.evaluate(() => ({
    hostId: globalThis.__browserPresence.hostId,
    bootId: globalThis.__browserPresence.bootId,
  }));
  await expect.poll(probe.output).toContain("unavailable reason=session-lost-for-return sequence=2");
  await expect.poll(probe.output).toContain("returned part=");
  await expect.poll(probe.output).toContain("returned-renewed sequence=4");
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.state())).toBe("admitted");
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.presenceState())).toBe("available");
  expect(await page.evaluate(() => ({
    hostId: globalThis.__browserPresence.hostId,
    bootId: globalThis.__browserPresence.bootId,
  }))).toEqual(identity);
  await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.freshnessProfile())).toMatchObject({
    sequence: 4,
    maximumReconnectAttempts: 1,
    backgroundRealtimeGuarantee: false,
  });
  await page.evaluate(() => globalThis.__browserPresence.close());
  await expect.poll(probe.output).toContain("returned-unavailable reason=session-lost");
  await expect.poll(() => probe.process.exitCode).toBe(0);
});

test("two admitted product clients compose Body grants into one exact ready session", async ({
  browser, context,
}, testInfo) => {
  const probe = await startWebRtcRendezvousProbe();
  const source = await context.newPage();
  const sink = await context.newPage();
  const browserErrors = [];
  for (const page of [source, sink]) {
    page.on("pageerror", (error) => browserErrors.push(error.message));
  }
  await Promise.all([
    source.goto(`/hosts/browser/browser-presence.test.html?renew=false&reconnect=false&body=${encodeURIComponent(probe.url)}`),
    sink.goto(`/hosts/browser/browser-presence.test.html?renew=false&reconnect=false&body=${encodeURIComponent(probe.url)}`),
  ]);
  for (const page of [source, sink]) {
    await expect.poll(() => page.evaluate(() => globalThis.__browserPresence?.presenceState()))
      .toBe("available");
    await expect.poll(() => page.evaluate(() => globalThis.__browserPresence.webRtcSessions()))
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
  await sourcePage.evaluate(() => globalThis.__browserPresence.close());
  await expect.poll(probe.output).toContain("peer-lost");
  await sinkPage.evaluate(() => globalThis.__browserPresence.close());
  await expect.poll(() => probe.process.exitCode).toBe(0);
  const basisMatch = probe.output().match(/^session_basis=(\{.*\})$/m);
  expect(basisMatch, probe.output()).not.toBeNull();
  const basis = JSON.parse(basisMatch[1]);
  const replacementBasisMatch = probe.output().match(/^replacement_basis=(\{.*\})$/m);
  expect(replacementBasisMatch, probe.output()).not.toBeNull();
  const replacementBasis = JSON.parse(replacementBasisMatch[1]);
  expect(basis).toMatchObject({
    base: "WebRtcDataChannel",
    session_limits: { maximum_in_flight_items: 1, maximum_payload_bytes: 16, maximum_buffered_bytes: 16 },
    line_limits: { maximum_in_flight_items: 1, maximum_payload_bytes: 16, maximum_buffered_bytes: 16 },
  });
  const evidenceRoot = process.env.CONDUIT_EVIDENCE_ROOT;
  if (testInfo.project.name === "chromium" && evidenceRoot) {
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
    const destination = path.join(evidenceRoot, "browser-webrtc-session.json");
    const temporary = `${destination}.tmp`;
    await writeFile(temporary, `${JSON.stringify(receipt, null, 2)}\n`);
    await rename(temporary, destination);
  }
});
