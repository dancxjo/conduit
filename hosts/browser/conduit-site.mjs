import { BrowserDomHost } from "./signal-dom-host.mjs";
import { BrowserWebSocketCarrier } from "./websocket-carrier.mjs";
import {
  instantiateDistributedToggleRuntime,
  runDistributedToggleRuntime,
} from "./distributed-toggle-runtime.mjs";

const MAXIMUM_RECEIPT_ITEMS = 16;
const MAXIMUM_RECEIPT_BYTES = 144;
const MAXIMUM_MESSAGE_BYTES = 2048;
const MAXIMUM_BUFFERED_BYTES = 8192;

function text(id, value) {
  const element = document.getElementById(id);
  if (element) element.textContent = value;
}

function shortIdentity(value) {
  if (typeof value !== "string" || value.length <= 28) return value ?? "—";
  return `${value.slice(0, 14)}…${value.slice(-10)}`;
}

function setConnection(status) {
  text("connection-status", status);
}

function presentReceipt(receipt, count) {
  document.documentElement.dataset.conduitLevel = String(receipt.level);
  text("signal-state", receipt.level ? "LIVE" : "QUIET");
  text("receipt-count", `${count} receipt${count === 1 ? "" : "s"}`);
  text("proof-status", "verified");
  text(
    "live-caption",
    `sequence ${receipt.sequence} completed by the browser host; level=${receipt.level}.`,
  );
  text("evidence-plan", shortIdentity(receipt.planId));
  text("evidence-fragment", shortIdentity(receipt.fragmentId));
  text("evidence-play", shortIdentity(receipt.activePlayId));
  text("evidence-placement", receipt.placementId);
  text("evidence-presentation", shortIdentity(receipt.presentationId));
  text("evidence-sequence", receipt.sequence);
  text("evidence-level", String(receipt.level));
}

async function run() {
  const params = new URLSearchParams(location.search);
  const url = params.get("ws");
  if (!url) {
    setConnection("waiting");
    text("proof-status", "no carrier");
    text(
      "live-caption",
      "Launch this page with `just site` so it receives an exact Conduit WebSocket endpoint.",
    );
    return;
  }

  try {
    setConnection("loading wasm");
    const wasmBytes = await fetch(
      "/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
    ).then((response) => {
      if (!response.ok) throw new Error("browser WASM artifact missing");
      return response.arrayBuffer();
    });

    setConnection("opening link");
    const carrier = await new BrowserWebSocketCarrier({
      url,
      maximumMessageBytes: MAXIMUM_MESSAGE_BYTES,
      maximumBufferedBytes: MAXIMUM_BUFFERED_BYTES,
    }).open();

    const domHost = new BrowserDomHost({
      hostId: "s4/toggle-browser-sink",
      bootId: "s4/toggle-browser-sink-boot",
      root: document.querySelector("#browser-sink"),
      maximumReceiptItems: MAXIMUM_RECEIPT_ITEMS,
      maximumReceiptBytes: MAXIMUM_RECEIPT_BYTES,
    });

    const runtime = await instantiateDistributedToggleRuntime(wasmBytes);
    let presentationCount = 0;
    const siteDomHost = {
      completePresentation(effect) {
        const result = domHost.completePresentation(effect);
        if (result.ok) {
          presentationCount += 1;
          presentReceipt(result.receipt, presentationCount);
        }
        return result;
      },
    };

    setConnection("session active");
    const result = await runDistributedToggleRuntime(runtime, carrier, siteDomHost);
    const closed = await carrier.closed();

    globalThis.__conduitSiteProof = Object.freeze({
      ...result,
      receipts: domHost.receipts(),
      closed,
    });

    setConnection("complete");
    text("proof-status", result.capacityStable ? "complete · bounded" : "complete · capacity changed");
  } catch (error) {
    document.documentElement.dataset.conduitLevel = "error";
    setConnection("error");
    text("proof-status", "failed");
    text("live-caption", `Conduit browser path failed: ${error.message ?? error}`);
    console.error(error);
  }
}

await run();
