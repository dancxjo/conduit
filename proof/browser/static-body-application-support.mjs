import assert from "node:assert/strict";
import { cp, mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { chromium } from "@playwright/test";
import { startStaticProduct } from "./static-product-server.mjs";

export async function stageStaticApplications() {
  const root = await mkdtemp(path.join(tmpdir(), "conduit-static-applications-"));
  let server;
  try {
    await cp("target/handbook-static", path.join(root, "handbook"), { recursive: true });
    await cp("target/handbook-static-second", path.join(root, "second"), { recursive: true });
    server = await startStaticProduct(root);
    return { url: server.url, async close() {
      await stop(server.child);
      await rm(root, { recursive: true });
    } };
  } catch (error) {
    if (server) await stop(server.child);
    await rm(root, { recursive: true });
    throw error;
  }
}

async function stop(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  await new Promise(resolve => {
    const timer = setTimeout(() => { child.kill("SIGKILL"); resolve(); }, 2_000);
    child.once("exit", () => { clearTimeout(timer); resolve(); });
    child.kill();
  });
}

export async function openStaticProfile(directory, origin) {
  await mkdir(directory, { recursive: true });
  const context = await chromium.launchPersistentContext(directory, {
    headless: true, viewport: { width: 1280, height: 900 }, serviceWorkers: "block",
  });
  context.setDefaultTimeout(15_000);
  context.setDefaultNavigationTimeout(30_000);
  const violations = [];
  const errors = [];
  await context.route("**/*", async route => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.origin !== origin || !["GET", "HEAD"].includes(request.method())) {
      violations.push(`${request.method()} ${request.url()}`);
      await route.abort("blockedbyclient");
    } else await route.continue();
  });
  await context.routeWebSocket("**/*", socket => {
    violations.push(`WebSocket ${socket.url()}`);
    socket.close({ code: 1008, reason: "Static application cannot require a live connection" });
  });
  await context.exposeBinding("__recordForbiddenStaticConnection", (_, kind) => violations.push(kind));
  await context.addInitScript(() => {
    // Deny peer connection establishment, not merely observation after signaling.
    // This is a network boundary guard; no application state or runtime is replaced.
    for (const name of ["RTCPeerConnection", "webkitRTCPeerConnection"]) {
      if (!globalThis[name]) continue;
      globalThis[name] = new Proxy(globalThis[name], { construct() {
        globalThis.__recordForbiddenStaticConnection(name);
        throw new DOMException("Static application peer connection forbidden", "SecurityError");
      } });
    }
  });
  const observe = page => page.on("pageerror", error => errors.push(error.message));
  context.pages().forEach(observe);
  context.on("page", observe);
  return { context, assertClean() {
    assert.deepEqual(violations, [], "Static application attempted a backend or external dependency");
    assert.deepEqual(errors, [], "Static application raised an uncaught browser error");
  } };
}
