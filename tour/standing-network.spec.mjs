import { expect, test } from "@playwright/test";

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

async function expectFamily(page, family) {
  await expect(page.locator("#patchbay-flow-root")).toHaveAttribute(
    "data-layout",
    "ready",
    { timeout: 60_000 },
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

  for (const family of ["network-link", "network-control", "network-state"]) {
    await expectFamily(page, family);
  }

  const source = page.locator("#source");
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
  for (const family of ["network-frame", "network-datagram", "network-stream"]) {
    await expectFamily(page, family);
    await expect(page.locator(`.patchbay-cord.type-family-${family}`).first()).toBeVisible();
  }

  await source.fill(`panel 0
source: net/packet/source { lifecycle = "standing" source = "10.0.0.2" destination = "10.1.0.2" hop_limit = 4 payload_bytes = 64 period_ticks = 10 maximum_packets_per_step = 1 maximum_packet_bytes = 1500 maximum_evidence_events = 64 }
sink: net/packet/sink { lifecycle = "standing" maximum_packets_per_step = 1 maximum_retained_items = 1 maximum_evidence_events = 64 }
source.packet > sink.packet { capacity = 2 max_value_bytes = 128 max_queued_bytes = 256 low_watermark = 0 high_watermark = 2 pressure = block }
`);
  await expectFamily(page, "network-packet");
  await expect(page.locator(".patchbay-cord.type-family-network-packet").first()).toBeVisible();

  await source.fill(`panel 0
listener: net/session/listen { lifecycle = "standing" transport = "tcp-reference" local_port = 8080 period_ticks = 10 session_timeout_ticks = 25 maximum_sessions = 8 maximum_retained_items = 8 maximum_evidence_events = 64 }
observe: net/observe/service { lifecycle = "standing" maximum_retained_items = 0 maximum_evidence_events = 64 }
listener.session > observe.session { capacity = 8 max_value_bytes = 64 max_queued_bytes = 512 low_watermark = 2 high_watermark = 8 pressure = block }
listener.event > observe.event { capacity = 8 max_value_bytes = 32 max_queued_bytes = 256 low_watermark = 2 high_watermark = 8 pressure = block }
listener.state > observe.state { capacity = 1 max_value_bytes = 32 max_queued_bytes = 32 low_watermark = 0 high_watermark = 1 pressure = block }
`);
  await expectFamily(page, "network-session");
  await expectFamily(page, "network-control");
  await expectFamily(page, "network-state");
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
