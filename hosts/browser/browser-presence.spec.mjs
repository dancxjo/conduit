import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";

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
    .rejects.toThrow("invalid WebRTC grant index");
  await page.evaluate(() => globalThis.__browserPresence.requestWebRtcGrant(0));
  await expect.poll(() => page.evaluate(() => globalThis.__browserWebRtcGrants.at(-1)))
    .toEqual({ kind: "web-rtc-grant", protocol: 1, index: 0, total: 0, grant: null });
  expect(await page.evaluate(() => Object.isFrozen(globalThis.__browserWebRtcGrants.at(-1))))
    .toBe(true);
  expect(await page.evaluate(() => {
    const source = {
      kind: "web-rtc-grant",
      protocol: 1,
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
    requests: [0],
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
  await expect.poll(probe.output).toContain("webrtc-grant index=0 total=0");
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

test("two admitted product clients compose Body grants into one exact ready session", async ({ context }) => {
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
  await source.evaluate(() => globalThis.__browserPresence.close());
  await expect.poll(() => source.evaluate(() => globalThis.__browserPresence.webRtcSessions()))
    .toMatchObject({ activeSessions: 0, terminalReason: "presence-closed" });
  await expect.poll(() => sink.evaluate(() => {
    const session = globalThis.__browserPresence.webRtcSessions().sessions[0];
    return session !== undefined && !session.sessionReady && session.terminalReason !== null;
  })).toBe(true);
  await expect.poll(probe.output).toContain("relayed stage=2");
  await expect.poll(probe.output).toContain("peer-lost");
  await sink.evaluate(() => globalThis.__browserPresence.close());
  await expect.poll(() => probe.process.exitCode).toBe(0);
});
