import assert from "node:assert/strict";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { chromium } from "@playwright/test";
import { startStaticProduct } from "./static-product-server.mjs";

export async function stageStaticApplications({ legacyLessonInventory = false, malformedLessonSource = false } = {}) {
  const root = await mkdtemp(path.join(tmpdir(), "conduit-static-applications-"));
  let server;
  let sourceInspection;
  try {
    await cp("target/handbook-static", path.join(root, "handbook"), { recursive: true });
    await cp("target/handbook-static-second", path.join(root, "second"), { recursive: true });
    if (legacyLessonInventory || malformedLessonSource) {
      // Produce a real compatible package whose Body is born with the earlier
      // two-lesson source inventory, using the ordinary package producer.
      const templateRoot = path.resolve("targets/browser/handbook");
      const template = JSON.parse(await readFile(path.join(templateRoot, "handbook.application.template.json"), "utf8"));
      const birth = JSON.parse(await readFile(path.join(templateRoot, "birth.json"), "utf8"));
      if (legacyLessonInventory) birth.plots = birth.plots.filter(plot =>
        ["tour", "patchbay", "clock-demo", "memory_lantern"].includes(plot.name));
      const birthPath = path.join(root, "legacy-birth.json");
      await writeFile(birthPath, JSON.stringify(birth));
      const lessons = birth.plots.filter(plot => plot.lesson);
      const broken = malformedLessonSource === "first" ? lessons[0]
        : malformedLessonSource === "middle" ? lessons[Math.floor(lessons.length / 2)]
          : birth.plots.find(plot => plot.name === "convert-pitch-demo");
      const brokenPath = path.join(root, "broken-lesson.conduit");
      sourceInspection = await Promise.all(birth.plots.map(async plot => {
        const resource = template.resources.find(resource => resource.role === plot.role);
        let source = await readFile(path.resolve(templateRoot, resource.source ?? resource.path), "utf8");
        if (malformedLessonSource && plot === broken) {
          const original = source;
          source = malformedLessonSource === true
            ? source.replace("source = 1kHz", 'source = "1kHz"')
            : source + '\nplot refused_fixture (\n angle: Angle = "1turn"\n) {\n}\n';
          assert.notEqual(source, original);
          await writeFile(brokenPath, source);
        }
        return { ...plot, source, malformed: Boolean(malformedLessonSource && plot === broken) };
      }));
      for (const resource of template.resources) {
        resource.source = resource.role === "birth-specification" ? birthPath
          : malformedLessonSource && resource.role === broken.role ? brokenPath
            : path.resolve(templateRoot, resource.source ?? resource.path);
      }
      const templatePath = path.join(root, "legacy-template.json");
      await writeFile(templatePath, JSON.stringify(template));
      execFileSync(process.execPath, ["targets/browser/tools/package-static-application.mjs",
        templatePath, "target/handbook-static/sdk/bundle", path.join(root, malformedLessonSource ? "broken" : "legacy")], { stdio: "pipe" });
    }
    server = await startStaticProduct(root);
    return { url: server.url, sourceInspection, async close() {
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
