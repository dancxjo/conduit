# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: static-body-application.spec.mjs >> static Handbook applications retain independent local Bodies through use, navigation, exclusion, reset and browser restart
- Location: work/face-mask-foundation/proof/browser/static-body-application.spec.mjs:29:1

# Error details

```
Error: ENOENT: no such file or directory, lstat 'target/handbook-static'
```

# Test source

```ts
  1  | import assert from "node:assert/strict";
  2  | import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
  3  | import { tmpdir } from "node:os";
  4  | import { execFileSync } from "node:child_process";
  5  | import path from "node:path";
  6  | import { chromium } from "@playwright/test";
  7  | import { startStaticProduct } from "./static-product-server.mjs";
  8  | 
  9  | export async function stageStaticApplications({ legacyLessonInventory = false } = {}) {
  10 |   const root = await mkdtemp(path.join(tmpdir(), "conduit-static-applications-"));
  11 |   let server;
  12 |   try {
> 13 |     await cp("target/handbook-static", path.join(root, "handbook"), { recursive: true });
     |     ^ Error: ENOENT: no such file or directory, lstat 'target/handbook-static'
  14 |     await cp("target/handbook-static-second", path.join(root, "second"), { recursive: true });
  15 |     if (legacyLessonInventory) {
  16 |       // Produce a real compatible package whose Body is born with the earlier
  17 |       // two-lesson source inventory, using the ordinary package producer.
  18 |       const templateRoot = path.resolve("targets/browser/handbook");
  19 |       const template = JSON.parse(await readFile(path.join(templateRoot, "handbook.application.template.json"), "utf8"));
  20 |       const birth = JSON.parse(await readFile(path.join(templateRoot, "birth.json"), "utf8"));
  21 |       birth.plots = birth.plots.filter(plot =>
  22 |         ["tour", "patchbay", "clock-demo", "memory_lantern"].includes(plot.name));
  23 |       const birthPath = path.join(root, "legacy-birth.json");
  24 |       await writeFile(birthPath, JSON.stringify(birth));
  25 |       for (const resource of template.resources) {
  26 |         resource.source = resource.role === "birth-specification" ? birthPath
  27 |           : path.resolve(templateRoot, resource.source ?? resource.path);
  28 |       }
  29 |       const templatePath = path.join(root, "legacy-template.json");
  30 |       await writeFile(templatePath, JSON.stringify(template));
  31 |       execFileSync(process.execPath, ["targets/browser/tools/package-static-application.mjs",
  32 |         templatePath, "target/handbook-static/sdk/bundle", path.join(root, "legacy")], { stdio: "pipe" });
  33 |     }
  34 |     server = await startStaticProduct(root);
  35 |     return { url: server.url, async close() {
  36 |       await stop(server.child);
  37 |       await rm(root, { recursive: true });
  38 |     } };
  39 |   } catch (error) {
  40 |     if (server) await stop(server.child);
  41 |     await rm(root, { recursive: true });
  42 |     throw error;
  43 |   }
  44 | }
  45 | 
  46 | async function stop(child) {
  47 |   if (child.exitCode !== null || child.signalCode !== null) return;
  48 |   await new Promise(resolve => {
  49 |     const timer = setTimeout(() => { child.kill("SIGKILL"); resolve(); }, 2_000);
  50 |     child.once("exit", () => { clearTimeout(timer); resolve(); });
  51 |     child.kill();
  52 |   });
  53 | }
  54 | 
  55 | export async function openStaticProfile(directory, origin) {
  56 |   await mkdir(directory, { recursive: true });
  57 |   const context = await chromium.launchPersistentContext(directory, {
  58 |     headless: true, viewport: { width: 1280, height: 900 }, serviceWorkers: "block",
  59 |   });
  60 |   context.setDefaultTimeout(15_000);
  61 |   context.setDefaultNavigationTimeout(30_000);
  62 |   const violations = [];
  63 |   const errors = [];
  64 |   await context.route("**/*", async route => {
  65 |     const request = route.request();
  66 |     const url = new URL(request.url());
  67 |     if (url.origin !== origin || !["GET", "HEAD"].includes(request.method())) {
  68 |       violations.push(`${request.method()} ${request.url()}`);
  69 |       await route.abort("blockedbyclient");
  70 |     } else await route.continue();
  71 |   });
  72 |   await context.routeWebSocket("**/*", socket => {
  73 |     violations.push(`WebSocket ${socket.url()}`);
  74 |     socket.close({ code: 1008, reason: "Static application cannot require a live connection" });
  75 |   });
  76 |   await context.exposeBinding("__recordForbiddenStaticConnection", (_, kind) => violations.push(kind));
  77 |   await context.addInitScript(() => {
  78 |     // Deny peer connection establishment, not merely observation after signaling.
  79 |     // This is a network boundary guard; no application state or runtime is replaced.
  80 |     for (const name of ["RTCPeerConnection", "webkitRTCPeerConnection"]) {
  81 |       if (!globalThis[name]) continue;
  82 |       globalThis[name] = new Proxy(globalThis[name], { construct() {
  83 |         globalThis.__recordForbiddenStaticConnection(name);
  84 |         throw new DOMException("Static application peer connection forbidden", "SecurityError");
  85 |       } });
  86 |     }
  87 |   });
  88 |   const observe = page => page.on("pageerror", error => errors.push(error.message));
  89 |   context.pages().forEach(observe);
  90 |   context.on("page", observe);
  91 |   return { context, assertClean() {
  92 |     assert.deepEqual(violations, [], "Static application attempted a backend or external dependency");
  93 |     assert.deepEqual(errors, [], "Static application raised an uncaught browser error");
  94 |   } };
  95 | }
  96 | 
```