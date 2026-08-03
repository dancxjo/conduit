import { expect, test } from "@playwright/test";

async function readLiveFlowReceipt(status) {
  return status.evaluate((element) => {
    const parseReceiptInteger = (value) => {
      const parsed = Number.parseInt(value, 10);
      return Number.isSafeInteger(parsed) ? parsed : null;
    };
    return {
      runId: element.dataset.runId || null,
      planIdentity: element.dataset.planIdentity || null,
      sourceRevision: parseReceiptInteger(element.dataset.sourceRevision),
      lastSequence: parseReceiptInteger(element.dataset.lastSequence),
      sourceIdentity: element.dataset.sourceIdentity || null,
      semanticSourceIdentity: element.dataset.semanticSourceIdentity || null,
      topologyIdentity: element.dataset.topologyIdentity || null,
    };
  });
}

async function readLayoutState(flowRoot) {
  return flowRoot.evaluate((element) => {
    const parseReceiptInteger = (value) => {
      const parsed = Number.parseInt(value, 10);
      return Number.isSafeInteger(parsed) ? parsed : null;
    };
    return {
      generation: Number.parseInt(element.dataset.layoutGeneration, 10),
      identity: element.dataset.layoutIdentity,
      planIdentity: element.dataset.activeEpoch || null,
      sourceIdentity: element.dataset.sourceIdentity || null,
      semanticSourceIdentity: element.dataset.semanticSourceIdentity || null,
      sourceRevision: parseReceiptInteger(element.dataset.candidateRevision),
      state: element.dataset.layout,
    };
  });
}

function hasMatchingLiveFlowSnapshot(receipt, layout, previousLayout = null) {
  const receiptSourceRevision = receipt?.sourceRevision;
  const layoutSourceRevision = layout?.sourceRevision;
  if (Number.isSafeInteger(receiptSourceRevision) &&
    Number.isSafeInteger(layoutSourceRevision) &&
    receiptSourceRevision !== layoutSourceRevision
  ) {
    return false;
  }

  const receiptSourceIdentity = receipt?.sourceIdentity;
  const layoutSourceIdentity = layout?.sourceIdentity;
  if (typeof receiptSourceIdentity === "string" && receiptSourceIdentity.length > 0) {
    if (typeof layoutSourceIdentity !== "string" || layoutSourceIdentity.length === 0) return false;
    if (receiptSourceIdentity !== layoutSourceIdentity) return false;
  }

  const receiptSemanticSourceIdentity = receipt?.semanticSourceIdentity;
  const layoutSemanticSourceIdentity = layout?.semanticSourceIdentity;
  if (typeof receiptSemanticSourceIdentity === "string" && receiptSemanticSourceIdentity.length > 0) {
    if (
      typeof layoutSemanticSourceIdentity !== "string" ||
      layoutSemanticSourceIdentity.length === 0
    ) return false;
    if (receiptSemanticSourceIdentity !== layoutSemanticSourceIdentity) return false;
  }

  const receiptTopologyIdentity = receipt?.topologyIdentity;
  const layoutTopologyIdentity = layout?.identity ?? layout?.topologyIdentity;
  if (typeof receiptTopologyIdentity === "string" && receiptTopologyIdentity.length > 0) {
    if (typeof layoutTopologyIdentity !== "string" || layoutTopologyIdentity.length === 0) return false;
    if (receiptTopologyIdentity !== layoutTopologyIdentity) return false;
  }

  const receiptPlanIdentity = receipt?.planIdentity;
  const layoutPlanIdentity = layout?.planIdentity;
  if (receiptPlanIdentity) {
    if (!layoutPlanIdentity || receiptPlanIdentity !== layoutPlanIdentity) return false;
  }

  const previousSourceRevision = previousLayout?.sourceRevision;
  if (Number.isSafeInteger(previousSourceRevision) &&
    Number.isSafeInteger(layoutSourceRevision) &&
    layoutSourceRevision > previousSourceRevision &&
    layout.sourceIdentity &&
    typeof previousLayout?.sourceIdentity === "string" &&
    layout.sourceIdentity === previousLayout.sourceIdentity
  ) {
    return false;
  }

  if (typeof layout?.semanticSourceIdentity === "string" &&
    layout.semanticSourceIdentity.length > 0 &&
    Number.isSafeInteger(previousSourceRevision) &&
    Number.isSafeInteger(layoutSourceRevision) &&
    layoutSourceRevision > previousSourceRevision &&
    typeof previousLayout?.semanticSourceIdentity === "string" &&
    layout.semanticSourceIdentity === previousLayout.semanticSourceIdentity
  ) {
    return false;
  }

  const previousTopologyIdentity = previousLayout?.identity;
  if (typeof previousTopologyIdentity === "string" &&
    previousTopologyIdentity.length > 0 &&
    Number.isSafeInteger(previousSourceRevision) &&
    Number.isSafeInteger(layoutSourceRevision) &&
    layoutSourceRevision > previousSourceRevision &&
    typeof layout?.identity === "string" &&
    layout.identity === previousTopologyIdentity
  ) {
    return false;
  }

  return true;
}

function hasReceiptSignals(receipt) {
  return Number.isSafeInteger(receipt?.sourceRevision) ||
    Number.isSafeInteger(receipt?.lastSequence) ||
    Boolean(receipt?.runId) ||
    Boolean(receipt?.planIdentity);
}

function isAuthoritativeAfter(previous, next, previousLayout = null, nextLayout = null) {
  if (!previous) return false;
  const hasPreviousSignals = hasReceiptSignals(previous);
  const hasNextSignals = hasReceiptSignals(next);
  if (!hasPreviousSignals || !hasNextSignals) {
    const beforeRevision = Number.isSafeInteger(previousLayout?.sourceRevision)
      ? previousLayout.sourceRevision
      : null;
    const afterRevision = Number.isSafeInteger(nextLayout?.sourceRevision)
      ? nextLayout.sourceRevision
      : null;
    if (beforeRevision !== null && afterRevision !== null) return afterRevision > beforeRevision;

    const beforePlan = previousLayout?.planIdentity || null;
    const afterPlan = nextLayout?.planIdentity || null;
    if (beforePlan && afterPlan) return afterPlan !== beforePlan;
    return false;
  }

  if (Number.isSafeInteger(previous.lastSequence) && Number.isSafeInteger(next.lastSequence)) {
    return next.lastSequence > previous.lastSequence;
  }

  if (Number.isSafeInteger(previous.lastSequence) !== Number.isSafeInteger(next.lastSequence)) {
    return false;
  }

  if (Number.isSafeInteger(previous.sourceRevision) &&
    Number.isSafeInteger(next.sourceRevision) &&
    next.sourceRevision > previous.sourceRevision
  ) {
    return true;
  }

  if (Boolean(previous.runId) && Boolean(next.runId) &&
    next.runId !== previous.runId) return true;

  if (Boolean(previous.planIdentity) && Boolean(next.planIdentity) &&
    next.planIdentity !== previous.planIdentity
  ) return true;

  return false;
}

async function waitForTopologyLayoutReady(
  flowRoot,
  status,
  afterGeneration = 0,
  afterIdentity = null,
  previousReceipt = null,
  previousLayout = null,
) {
  await expect.poll(async () => {
    const layout = await readLayoutState(flowRoot);
    const receipt = await readLiveFlowReceipt(status);
    const hasAuthoritativeTransition = previousReceipt
      ? isAuthoritativeAfter(previousReceipt, receipt, previousLayout, layout)
      : true;
    const hasMatchingSnapshot = previousReceipt
      ? hasMatchingLiveFlowSnapshot(receipt, layout, previousLayout)
      : true;
    return layout.state === "ready" &&
      layout.generation > afterGeneration &&
      layout.identity !== afterIdentity &&
      hasAuthoritativeTransition &&
      hasMatchingSnapshot &&
      typeof layout.identity === "string" &&
      layout.identity.length > 0
      ? layout
      : null;
  }, { timeout: 60_000 }).not.toBeNull();
  const layout = await readLayoutState(flowRoot);
  const receipt = await readLiveFlowReceipt(status);
  expect(layout.state).toBe("ready");
  expect(layout.generation).toBeGreaterThan(afterGeneration);
  expect(layout.sourceIdentity).toEqual(expect.any(String));
  expect(layout.sourceIdentity.length).toBeGreaterThan(0);
  expect(layout.semanticSourceIdentity).toEqual(expect.any(String));
  expect(layout.semanticSourceIdentity.length).toBeGreaterThan(0);
  expect(layout.identity).toEqual(expect.any(String));
  expect(layout.identity.length).toBeGreaterThan(0);
  if (previousReceipt) {
    expect(isAuthoritativeAfter(previousReceipt, receipt, previousLayout, layout)).toBeTruthy();
    expect(hasMatchingLiveFlowSnapshot(receipt, layout, previousLayout)).toBeTruthy();
  }
  return layout;
}

async function gotoStandingNetwork(page) {
  const path = "/tour/public/index.html?lesson=library.bounded-brainstem-network";
  try {
    await page.goto(path);
  } catch (error) {
    if (!String(error).includes("is interrupted by another navigation")) throw error;
  }
  await expect(page.locator("html")).toHaveAttribute("data-tour-ready", "true", {
    timeout: 20_000,
  });
}

async function expectFamily(page, family, options = {}) {
  const flowRoot = options.flowRoot ?? page.locator("#patchbay-flow-root");
  const status = options.status ?? page.locator("#live-flow-status");
  await waitForTopologyLayoutReady(
    flowRoot,
    status,
    0,
  );
  const row = page.locator(`.faceplate-port-row[data-signal-family="${family}"]`).first();
  await expect(row, `${family} port is projected`).toBeVisible();
  await expect(row.locator(".jack-label")).toHaveAttribute(
    "title",
    new RegExp(`${family} signal`),
  );
  await expect(row.locator(".jack-handle")).toHaveAttribute(
    "data-signal-family",
    family,
  );
}

test("distinguishes every standing network value family without color alone", async ({ page }) => {
  await gotoStandingNetwork(page);
  const flowRoot = page.locator("#patchbay-flow-root");
  const liveFlowStatus = page.locator("#live-flow-status");
  let layout = await waitForTopologyLayoutReady(flowRoot, liveFlowStatus, 0);

  for (const family of ["network-link", "network-control", "network-state"]) {
    await expectFamily(page, family);
  }

  const source = page.locator("#source");
  let priorReceipt = await readLiveFlowReceipt(liveFlowStatus);
  await source.fill(`panel 0
frame_source: net/frame/source { lifecycle = "standing" interface = 1 period_ticks = 1000 payload_bytes = 64 maximum_frame_bytes = 1518 maximum_evidence_events = 64 }
frame_sink: net/frame/sink { lifecycle = "standing" maximum_frames_per_step = 1 maximum_retained_items = 1 maximum_evidence_events = 64 }
datagram_source: net/datagram/source { lifecycle = "standing" source_port = 30000 destination_port = 30001 period_ticks = 1000 payload_bytes = 64 maximum_datagram_bytes = 1472 maximum_datagrams_per_step = 1 maximum_evidence_events = 64 }
datagram_sink: net/datagram/sink { lifecycle = "standing" maximum_datagrams_per_step = 1 maximum_retained_items = 1 maximum_evidence_events = 64 }
stream_source: net/stream/source { lifecycle = "standing" session = 1 period_ticks = 1000 chunk_bytes = 64 maximum_chunk_bytes = 1024 maximum_evidence_events = 64 }
stream_sink: net/stream/sink { lifecycle = "standing" maximum_chunks_per_step = 1 maximum_retained_items = 1 maximum_evidence_events = 64 }
frame_source.frame > frame_sink.frame { capacity = 2 max_value_bytes = 128 max_queued_bytes = 256 low_watermark = 0 high_watermark = 2 pressure = block }
datagram_source.datagram > datagram_sink.datagram { capacity = 2 max_value_bytes = 128 max_queued_bytes = 256 low_watermark = 0 high_watermark = 2 pressure = block }
stream_source.chunk > stream_sink.chunk { capacity = 2 max_value_bytes = 96 max_queued_bytes = 192 low_watermark = 0 high_watermark = 2 pressure = block }
  `);
  layout = await waitForTopologyLayoutReady(
    flowRoot,
    liveFlowStatus,
    layout.generation,
    layout.identity,
    priorReceipt,
    layout,
  );
  priorReceipt = await readLiveFlowReceipt(liveFlowStatus);
  for (const family of ["network-frame", "network-datagram", "network-stream"]) {
    await expectFamily(page, family, { flowRoot, status: liveFlowStatus });
    await expect(page.locator(`.patchbay-cord.type-family-${family}`).first()).toBeVisible();
  }

  await source.fill(`panel 0
source: net/packet/source { lifecycle = "standing" source = "10.0.0.2" destination = "10.1.0.2" hop_limit = 4 payload_bytes = 64 period_ticks = 10 maximum_packets_per_step = 1 maximum_packet_bytes = 1500 maximum_evidence_events = 64 }
sink: net/packet/sink { lifecycle = "standing" maximum_packets_per_step = 1 maximum_retained_items = 1 maximum_evidence_events = 64 }
source.packet > sink.packet { capacity = 2 max_value_bytes = 128 max_queued_bytes = 256 low_watermark = 0 high_watermark = 2 pressure = block }
  `);
  layout = await waitForTopologyLayoutReady(
    flowRoot,
    liveFlowStatus,
    layout.generation,
    layout.identity,
    priorReceipt,
    layout,
  );
  priorReceipt = await readLiveFlowReceipt(liveFlowStatus);
  await expectFamily(page, "network-packet", { flowRoot, status: liveFlowStatus });
  await expect(page.locator(".patchbay-cord.type-family-network-packet").first()).toBeVisible();

  await source.fill(`panel 0
listener: net/session/listen { lifecycle = "standing" transport = "tcp-reference" local_port = 8080 period_ticks = 10 session_timeout_ticks = 25 maximum_sessions = 8 maximum_retained_items = 8 maximum_evidence_events = 64 }
observe: net/observe/service { lifecycle = "standing" maximum_retained_items = 0 maximum_evidence_events = 64 }
listener.session > observe.session { capacity = 8 max_value_bytes = 64 max_queued_bytes = 512 low_watermark = 2 high_watermark = 8 pressure = block }
listener.event > observe.event { capacity = 8 max_value_bytes = 32 max_queued_bytes = 256 low_watermark = 2 high_watermark = 8 pressure = block }
listener.state > observe.state { capacity = 1 max_value_bytes = 32 max_queued_bytes = 32 low_watermark = 0 high_watermark = 1 pressure = block }
  `);
  await waitForTopologyLayoutReady(
    flowRoot,
    liveFlowStatus,
    layout.generation,
    layout.identity,
    priorReceipt,
    layout,
  );
  await expectFamily(page, "network-session", { flowRoot, status: liveFlowStatus });
  await expectFamily(page, "network-control", { flowRoot, status: liveFlowStatus });
  await expectFamily(page, "network-state", { flowRoot, status: liveFlowStatus });
  await expect(page.locator(".patchbay-cord.type-family-network-session").first()).toBeVisible();
  await expect(page.locator(".patchbay-cord.type-family-network-control").first()).toBeVisible();
  await expect(page.locator(".patchbay-cord.type-family-network-state").first()).toBeVisible();
});

test("states the standing-network contrast and non-authority boundaries", async ({ page }) => {
  await gotoStandingNetwork(page);
  await expect(page.locator("#prose")).toContainText("standing semantic network graph");
  await expect(page.locator("#prose")).toContainText("not an imperative setup script");
  await expect(page.locator("#prose")).toContainText("No route installation, bridge, forwarding, NAT, firewall, Internet");
});
