# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: static-body-application.spec.mjs >> static Handbook applications retain independent local Bodies through use, navigation, exclusion, reset and browser restart
- Location: proof/browser/static-body-application.spec.mjs:53:1

# Error details

```
Error: ENOENT: no such file or directory, lstat 'target/handbook-static'
```

# Test source

```ts
  1   | import assert from "node:assert/strict";
  2   | import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
  3   | import { tmpdir } from "node:os";
  4   | import { execFileSync } from "node:child_process";
  5   | import path from "node:path";
  6   | import { chromium } from "@playwright/test";
  7   | import { startStaticProduct } from "./static-product-server.mjs";
  8   | 
  9   | export async function stageStaticApplications({ legacyLessonInventory = false, malformedLessonSource = false } = {}) {
  10  |   const root = await mkdtemp(path.join(tmpdir(), "conduit-static-applications-"));
  11  |   let server;
  12  |   try {
> 13  |     await cp("target/handbook-static", path.join(root, "handbook"), { recursive: true });
      |     ^ Error: ENOENT: no such file or directory, lstat 'target/handbook-static'
  14  |     await cp("target/handbook-static-second", path.join(root, "second"), { recursive: true });
  15  |     if (legacyLessonInventory || malformedLessonSource) {
  16  |       // Produce a real compatible package whose Body is born with the earlier
  17  |       // two-lesson source inventory, using the ordinary package producer.
  18  |       const templateRoot = path.resolve("targets/browser/handbook");
  19  |       const template = JSON.parse(await readFile(path.join(templateRoot, "handbook.application.template.json"), "utf8"));
  20  |       const birth = JSON.parse(await readFile(path.join(templateRoot, "birth.json"), "utf8"));
  21  |       if (legacyLessonInventory) birth.plots = birth.plots.filter(plot =>
  22  |         ["tour", "patchbay", "clock-demo", "memory_lantern"].includes(plot.name));
  23  |       const birthPath = path.join(root, "legacy-birth.json");
  24  |       await writeFile(birthPath, JSON.stringify(birth));
  25  |       const originalPitch = await readFile(path.join(templateRoot, "plots/convert-pitch.conduit"), "utf8");
  26  |       const brokenPitchPath = path.join(root, "broken-pitch.conduit");
  27  |       if (malformedLessonSource) {
  28  |         const malformed = originalPitch.replace("source = 1kHz", 'source = "1kHz"');
  29  |         assert.notEqual(malformed, originalPitch);
  30  |         await writeFile(brokenPitchPath, malformed);
  31  |       }
  32  |       for (const resource of template.resources) {
  33  |         resource.source = resource.role === "birth-specification" ? birthPath
  34  |           : malformedLessonSource && resource.role === "lesson-convert-pitch-source" ? brokenPitchPath
  35  |             : path.resolve(templateRoot, resource.source ?? resource.path);
  36  |       }
  37  |       const templatePath = path.join(root, "legacy-template.json");
  38  |       await writeFile(templatePath, JSON.stringify(template));
  39  |       execFileSync(process.execPath, ["targets/browser/tools/package-static-application.mjs",
  40  |         templatePath, "target/handbook-static/sdk/bundle", path.join(root, malformedLessonSource ? "broken" : "legacy")], { stdio: "pipe" });
  41  |     }
  42  |     server = await startStaticProduct(root);
  43  |     return { url: server.url, async close() {
  44  |       await stop(server.child);
  45  |       await rm(root, { recursive: true });
  46  |     } };
  47  |   } catch (error) {
  48  |     if (server) await stop(server.child);
  49  |     await rm(root, { recursive: true });
  50  |     throw error;
  51  |   }
  52  | }
  53  | 
  54  | async function stop(child) {
  55  |   if (child.exitCode !== null || child.signalCode !== null) return;
  56  |   await new Promise(resolve => {
  57  |     const timer = setTimeout(() => { child.kill("SIGKILL"); resolve(); }, 2_000);
  58  |     child.once("exit", () => { clearTimeout(timer); resolve(); });
  59  |     child.kill();
  60  |   });
  61  | }
  62  | 
  63  | export async function openStaticProfile(directory, origin) {
  64  |   await mkdir(directory, { recursive: true });
  65  |   const context = await chromium.launchPersistentContext(directory, {
  66  |     headless: true, viewport: { width: 1280, height: 900 }, serviceWorkers: "block",
  67  |   });
  68  |   context.setDefaultTimeout(15_000);
  69  |   context.setDefaultNavigationTimeout(30_000);
  70  |   const violations = [];
  71  |   const errors = [];
  72  |   await context.route("**/*", async route => {
  73  |     const request = route.request();
  74  |     const url = new URL(request.url());
  75  |     if (url.origin !== origin || !["GET", "HEAD"].includes(request.method())) {
  76  |       violations.push(`${request.method()} ${request.url()}`);
  77  |       await route.abort("blockedbyclient");
  78  |     } else await route.continue();
  79  |   });
  80  |   await context.routeWebSocket("**/*", socket => {
  81  |     violations.push(`WebSocket ${socket.url()}`);
  82  |     socket.close({ code: 1008, reason: "Static application cannot require a live connection" });
  83  |   });
  84  |   await context.exposeBinding("__recordForbiddenStaticConnection", (_, kind) => violations.push(kind));
  85  |   await context.addInitScript(() => {
  86  |     // Deny peer connection establishment, not merely observation after signaling.
  87  |     // This is a network boundary guard; no application state or runtime is replaced.
  88  |     for (const name of ["RTCPeerConnection", "webkitRTCPeerConnection"]) {
  89  |       if (!globalThis[name]) continue;
  90  |       globalThis[name] = new Proxy(globalThis[name], { construct() {
  91  |         globalThis.__recordForbiddenStaticConnection(name);
  92  |         throw new DOMException("Static application peer connection forbidden", "SecurityError");
  93  |       } });
  94  |     }
  95  |   });
  96  |   const observe = page => page.on("pageerror", error => errors.push(error.message));
  97  |   context.pages().forEach(observe);
  98  |   context.on("page", observe);
  99  |   return { context, assertClean() {
  100 |     assert.deepEqual(violations, [], "Static application attempted a backend or external dependency");
  101 |     assert.deepEqual(errors, [], "Static application raised an uncaught browser error");
  102 |   } };
  103 | }
  104 | 
```