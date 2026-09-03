import { expect, test } from "@playwright/test";
import {
  createFireflySynchronizer,
  decodePulseObservation,
} from "../../targets/browser/host/assets/firefly-choir.mjs";

async function openHosts(context) {
  const left = await context.newPage();
  const right = await context.newPage();
  await Promise.all([
    left.goto("/proof/browser/firefly-choir.test.html"),
    right.goto("/proof/browser/firefly-choir.test.html"),
  ]);
  await Promise.all([
    left.evaluate(() => window.fireflyCreate({ enableTone: true })),
    right.evaluate(() => window.fireflyCreate({ enableTone: false })),
  ]);
  await Promise.all([
    left.evaluate(() => window.fireflyMarkLinked()),
    right.evaluate(() => window.fireflyMarkLinked()),
  ]);
  return { left, right };
}

function queueOutbound(sender, receiver, queues, sendAtMs, receiveAtMs) {
  sender.advance(sendAtMs);
  for (const bytes of sender.takeOutbound()) {
    queues.push({ receiver, bytes, receiveAtMs });
  }
}

function deliverQueues(queues, nowMs) {
  let delivered = 0;
  for (const pending of [...queues]) {
    if (pending.receiveAtMs !== nowMs) continue;
    pending.receiver.ingest(0, Uint8Array.from(pending.bytes), nowMs);
    queues.splice(queues.indexOf(pending), 1);
    delivered += 1;
  }
  return delivered;
}

function localPulseTimes(sync) {
  return sync.snapshot().history
    .filter((entry) => entry.kind === "local")
    .map((entry) => entry.atMs);
}

test("deterministic bounded synchronization converges without requiring audio", async () => {
  const left = createFireflySynchronizer();
  const right = createFireflySynchronizer({ enableTone: false });
  const queues = [];
  left.tap(0);
  queues.push(...left.takeOutbound().map((bytes) => ({ receiver: right, bytes, receiveAtMs: 15 })));
  right.tap(120);
  queues.push(...right.takeOutbound().map((bytes) => ({ receiver: left, bytes, receiveAtMs: 135 })));

  for (let nowMs = 0; nowMs <= 1_440; nowMs += 15) {
    deliverQueues(queues, nowMs);
    queueOutbound(left, right, queues, nowMs, nowMs + 15);
    queueOutbound(right, left, queues, nowMs, nowMs + 15);
  }
  while (queues.length > 0) {
    const nextAtMs = Math.min(...queues.map((item) => item.receiveAtMs));
    deliverQueues(queues, nextAtMs);
  }

  const leftPulses = localPulseTimes(left);
  const rightPulses = localPulseTimes(right);
  expect(leftPulses.length).toBeGreaterThanOrEqual(4);
  expect(rightPulses.length).toBeGreaterThanOrEqual(4);
  expect(Math.abs(leftPulses.at(-1) - rightPulses.at(-1))).toBeLessThan(120);
  expect(Math.abs(left.snapshot().periodMs - right.snapshot().periodMs)).toBeLessThanOrEqual(2);
  expect(left.snapshot().peerStates[0].accepted).toBeGreaterThanOrEqual(4);
  expect(right.snapshot().peerStates[0].accepted).toBeGreaterThanOrEqual(4);
  expect(left.snapshot().enableTone).toBe(true);
  expect(right.snapshot().enableTone).toBe(false);

  const pressured = createFireflySynchronizer({ maximumQueuedObservations: 1 });
  pressured.tap(0);
  pressured.advance(300);
  expect(pressured.snapshot().pressureCount).toBe(1);
  const observation = decodePulseObservation(Uint8Array.from(pressured.takeOutbound()[0]));
  expect(observation).toEqual({ sequence: 0, periodMs: 240 });
});

test("two browser Hosts exchange bounded pulse observations, converge, and expose line loss", async ({
  context,
}) => {
  const { left, right } = await openHosts(context);
  await left.evaluate(() => window.fireflySetTime(0));
  await left.getByRole("button", { name: "Tap rhythm" }).click();
  const [leftBytes] = await left.evaluate(() => window.fireflyTakeOutbound());
  expect(leftBytes.length).toBe(6);

  await right.evaluate(() => window.fireflySetTime(15));
  await right.evaluate((bytes) => window.fireflyReceive(bytes), leftBytes);
  await expect(right.locator("#status")).toContainText("audio=omitted");

  await right.evaluate(() => window.fireflySetTime(120));
  await right.getByRole("button", { name: "Tap rhythm" }).focus();
  await right.keyboard.press("Space");
  const [rightBytes] = await right.evaluate(() => window.fireflyTakeOutbound());
  expect(rightBytes.length).toBe(6);

  await left.evaluate(() => window.fireflySetTime(135));
  await left.evaluate((bytes) => window.fireflyReceive(bytes), rightBytes);

  for (const nowMs of [240, 360, 480, 600, 720, 840, 960]) {
    await left.evaluate((value) => window.fireflySetTime(value), nowMs);
    await right.evaluate((value) => window.fireflySetTime(value), nowMs);
    await left.evaluate(() => window.fireflyAdvance());
    await right.evaluate(() => window.fireflyAdvance());

    const leftSent = await left.evaluate(() => window.fireflyTakeOutbound());
    for (const bytes of leftSent) {
      await right.evaluate((value) => window.fireflySetTime(value), nowMs + 15);
      await right.evaluate((value) => window.fireflyReceive(value), bytes);
    }

    const rightSent = await right.evaluate(() => window.fireflyTakeOutbound());
    for (const bytes of rightSent) {
      await left.evaluate((value) => window.fireflySetTime(value), nowMs + 15);
      await left.evaluate((value) => window.fireflyReceive(value), bytes);
    }
  }

  const [leftState, rightState] = await Promise.all([
    left.evaluate(() => window.fireflyState()),
    right.evaluate(() => window.fireflyState()),
  ]);
  expect(leftState.lineState).toBe("linked");
  expect(rightState.lineState).toBe("linked");
  expect(leftState.tones.length).toBeGreaterThan(0);
  expect(rightState.tones).toEqual([]);
  expect(Math.abs(leftState.lastPulseAtMs - rightState.lastPulseAtMs)).toBeLessThan(120);
  expect(Math.abs(leftState.periodMs - rightState.periodMs)).toBeLessThanOrEqual(2);
  expect(leftState.peerStates[0].accepted).toBeGreaterThanOrEqual(3);
  expect(rightState.peerStates[0].accepted).toBeGreaterThanOrEqual(3);
  expect(leftState.history.length).toBeLessThanOrEqual(16);
  expect(rightState.history.length).toBeLessThanOrEqual(16);
  await expect(left.locator("#light")).toContainText(/[●○]/);
  await expect(right.locator("#light")).toContainText(/[●○]/);

  await right.evaluate(() => window.fireflyMarkLineClosed({ reason: "line-lost" }));
  await expect(right.locator("#status")).toContainText("line=lost");
  await context.close();
});
