import { chromium, expect, test } from "@playwright/test";
import { startBrowserHumanBodyCapstone } from "./browser-presence-support.mjs";

test("two browser Hosts realize camera-summary only after exact acquired resource truth", async () => {
  const body = await startBrowserHumanBodyCapstone();
  const browser = await chromium.launch({
    headless: true,
    args: ["--use-fake-device-for-media-stream", "--use-fake-ui-for-media-stream"],
  });
  const context = await browser.newContext();
  const hostPort = process.env.CONDUIT_BROWSER_HOST_PORT ?? "4173";
  await context.grantPermissions(["camera"], { origin: `http://127.0.0.1:${hostPort}` });
  const source = await context.newPage();
  const sink = await context.newPage();
  const errors = [];
  for (const page of [source, sink]) page.on("pageerror", (error) => errors.push(error.message));
  try {
    await Promise.all([source, sink].map((page) => page.goto(
      `/hosts/browser/browser-presence.test.html?renew=false&reconnect=false&body=${encodeURIComponent(body.url)}`,
    )));
    for (const page of [source, sink]) {
      await expect.poll(async () => ({
        presence: await page.evaluate(() => globalThis.__browserPresence?.presenceState()),
        state: await page.evaluate(() => globalThis.__browserPresence?.state()),
        status: await page.locator("p[role=status]").textContent(),
        errors,
        body: body.output(),
        exitCode: body.process.exitCode,
      })).toMatchObject({ presence: "available" });
    }
    expect(body.output()).not.toContain("planned=");
    await source.getByRole("button", { name: "Acquire camera resource" }).click();
    await expect.poll(() => source.evaluate(() => ({
      plan: globalThis.__browserHumanMedia.plan(),
      evidence: globalThis.__browserHumanMedia.evidence(),
      mediaStatus: document.querySelector("#media-status").textContent,
    })), {
      message: `browser errors=${errors.join(" | ")} body=${body.output()}`,
      timeout: 15_000,
    }).toMatchObject({ plan: { outputPort: "frame" } });
    await expect.poll(() => body.output()).toContain("planned=");
    for (const page of [source, sink]) {
      await expect.poll(() => page.evaluate(
        () => globalThis.__browserPresence.webRtcSessions(),
      ), { message: body.output(), timeout: 15_000 }).toMatchObject({
        activeSessions: 1,
        sessions: [{ sessionReady: true }],
        failure: null,
      });
    }
    const sourceSession = await source.evaluate(
      () => globalThis.__browserPresence.webRtcSessions().sessions[0],
    );
    const sinkSession = await sink.evaluate(
      () => globalThis.__browserPresence.webRtcSessions().sessions[0],
    );
    expect(sourceSession.role).toBe("source");
    expect(sinkSession.role).toBe("sink");
    await expect(source.evaluate(
      (id) => globalThis.__browserPresence.offerWebRtcValue(id, new Array(65_537).fill(1)),
      sourceSession.negotiationId,
    )).rejects.toThrow("Cord offer refused");
    const receive = sink.evaluate(
      (id) => globalThis.__browserPresence.receiveWebRtcValue(id),
      sinkSession.negotiationId,
    );
    await source.getByRole("button", { name: "Use planned camera resource" }).click();
    const received = await receive;
    expect(received.sequence).toBe(0);
    expect(received.bytes.length).toBeGreaterThan(0);
    expect(received.bytes.length).toBeLessThanOrEqual(64 * 1024);
    const plan = await source.evaluate(() => globalThis.__browserHumanMedia.plan());
    const evidence = await source.evaluate(() => globalThis.__browserHumanMedia.evidence());
    const traffic = await source.evaluate(() => globalThis.__browserHumanMedia.traffic());
    expect(evidence).toMatchObject({
      use_plan_id: plan.planId,
      output_port: "frame",
      phase: "terminal",
      terminal: "MediaClosed",
    });
    expect(evidence.acquisition_plan_id).not.toBe(plan.planId);
    expect(evidence.last_value_bytes).toBe(received.bytes.length);
    expect(traffic).toMatchObject({
      negotiationId: sourceSession.negotiationId,
      accepted: { accepted: true, sequence: 0 },
    });
    const delivered = source.evaluate(
      ({ id, sequence }) => globalThis.__browserPresence.waitWebRtcValueDelivered(id, sequence),
      { id: sourceSession.negotiationId, sequence: received.sequence },
    );
    await sink.evaluate(
      ({ id, sequence }) => globalThis.__browserPresence.deliverWebRtcValue(id, sequence),
      { id: sinkSession.negotiationId, sequence: received.sequence },
    );
    await delivered;
    await source.evaluate(() => globalThis.__browserPresence.close());
    await source.close();
    await expect.poll(() => body.output()).toContain("host_loss=");
    const loss = JSON.parse(body.output().match(/^host_loss=(\{.*\})$/m)[1]);
    expect(loss).toMatchObject({
      index: 0,
      form: "camera-summary",
      replacement: "unrealizable-without-new-camera-resource-truth",
    });
    await expect.poll(() => body.process.exitCode).toBe(0);
    expect(errors).toEqual([]);
  } finally {
    if (!sink.isClosed()) await sink.evaluate(() => globalThis.__browserPresence.close()).catch(() => {});
    await context.close();
    await browser.close();
    if (body.process.exitCode === null) body.process.kill();
  }
});
