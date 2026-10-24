# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: patchbay-authoring.spec.mjs >> catalog queries and two durable layouts decorate the same live Plot
- Location: proof/browser/patchbay-authoring.spec.mjs:27:3

# Error details

```
Error: response.body: Protocol error (Network.getResponseBody): Request content was evicted from inspector cache
Response body is not available for a response that was navigated away from. Read response.body() before triggering any navigation.
```

```
Error: response.body: Protocol error (Network.getResponseBody): Request content was evicted from inspector cache
Response body is not available for a response that was navigated away from. Read response.body() before triggering any navigation.
```

# Page snapshot

```yaml
- generic [active] [ref=e1]:
  - link "Skip to main content" [ref=e2] [cursor=pointer]:
    - /url: "#main-content"
  - status [ref=e3]:
    - generic [ref=e4]:
      - strong [ref=e6]: Conduit Patchbay
      - generic [ref=e7]: Preparing the workbench…
  - generic:
    - navigation "Patchbay workspace":
      - generic "Patchbay surfaces":
        - button "Library"
        - button "Inspect"
        - button "No body"
    - main:
      - heading "Conduit Patchbay" [level=1]
      - generic:
        - region "Entrance choices":
          - generic [ref=e8]:
            - generic [ref=e9]:
              - paragraph [ref=e10]: ENTRANCE · STRUCTURE
              - heading "Entrance choices" [level=2] [ref=e11]
            - generic "Canvas actions" [ref=e13]:
              - button "Fit" [ref=e14] [cursor=pointer]
              - button "Center selected" [disabled] [ref=e15]
              - button "Zoom in" [ref=e16] [cursor=pointer]
              - button "Zoom out" [ref=e17] [cursor=pointer]
              - button "Pan right" [ref=e18] [cursor=pointer]
              - button "Arrange" [ref=e19] [cursor=pointer]
              - generic [ref=e20]:
                - text: Look
                - combobox "Look" [ref=e21] [cursor=pointer]:
                  - option "Phosphor" [selected]
                  - option "Editorial"
                  - option "Engineering"
              - button "High contrast" [ref=e22] [cursor=pointer]
          - region "Patchbay spatial workspace"
        - complementary:
          - heading "Inspect" [level=2]
          - paragraph: Select something on the canvas or in Library to see its state, realization, dependencies, and failures.
          - region "Execution control":
            - heading "Execution control" [level=3]
          - region "Observation timeline":
            - heading "Observation timeline" [level=3]
            - generic "Exact retained debugger events"
          - region "Watches":
            - heading "Watches" [level=3]
            - generic "Bounded debugger Watches"
            - generic "Bounded Watch history"
          - group:
            - generic "Related subjects"
          - group:
            - generic "Exact details"
      - region "Workspace status and tools":
        - group:
          - generic "Workspace layouts and annotations (face only)"
        - region "Current run and diagnostic status":
          - strong: Current truth
          - generic: Loading…
          - generic: Diagnostics loading…
        - region "Current plot realization":
          - strong: Run status
      - group:
        - generic "Advanced evidence and linear face"
```

# Test source

```ts
  1   | import { createHash } from "node:crypto";
  2   | const observedWasm = [];
  3   | const wasmReads = [];
  4   | test.beforeEach(async ({ page }) => {
  5   |   page.on("response", response => {
  6   |     if (response.url().includes(".wasm") || (response.headers()["content-type"] || "").includes("application/wasm")) {
> 7   |       wasmReads.push((async () => { const body = await response.body(); observedWasm.push({url:response.url(),status:response.status(),bytes:body.length,sha256:createHash("sha256").update(body).digest("hex")}); })());
      |                                                                 ^ Error: response.body: Protocol error (Network.getResponseBody): Request content was evicted from inspector cache
  8   |     }
  9   |   });
  10  | });
  11  | test.afterEach(async ({}, testInfo) => { await Promise.all(wasmReads); await writeFile(testInfo.outputPath("observed-wasm-network.json"), JSON.stringify(observedWasm,null,2)); });
  12  | import { spawn } from "node:child_process";
  13  | import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
  14  | import { tmpdir } from "node:os";
  15  | import { join } from "node:path";
  16  | import { createInterface } from "node:readline";
  17  | import { expect, test } from "@playwright/test";
  18  | 
  19  | async function startAuthoringEntrance(plotSource = "plot making {\n}\n") {
  20  |   const directory = await mkdtemp(join(tmpdir(), "conduit-browser-authoring-"));
  21  |   const source = join(directory, "making.conduit");
  22  |   await writeFile(source, plotSource);
  23  |   return { ...(await spawnEntrance(source)), directory, source };
  24  | }
  25  | 
  26  | for (const includeLifecycle of [false, true]) {
  27  |   test(includeLifecycle
  28  |     ? "catalog queries and two durable layouts decorate the same live Plot"
  29  |     : "unbodied catalog and durable layouts preserve the exact source Face", async ({ page }, testInfo) => {
  30  |     test.setTimeout(90_000);
  31  |     const source = `plot authoring {
  32  |       literal: text/literal("authoring truth")
  33  |       prefix: text/join("First: ")
  34  |       upper: text/upper
  35  |       label: text/join("Result: ")
  36  |       display: face/text(label = "Message", maximum-values = 4)
  37  |       scalar: scalar/literal(value = 500000)
  38  |       map: math/map-quantity
  39  |       quantity: face/quantity("Measured quantity")
  40  |       wrapped: structured-info/wrap-quantity
  41  |       literal >> prefix >> upper >> label >> display
  42  |       scalar.value >> map.in
  43  |       map >> wrapped >> quantity
  44  |     }\n`;
  45  |     const server = await startAuthoringEntrance(source);
  46  |     try {
  47  |       await page.goto(server.url);
  48  |       await page.getByRole("button", { name: "Open Plot Empty Plot" }).click();
  49  |       await expect(page.locator(".flow-frontplate.role-gear")).toHaveCount(9);
  50  |       const initial = await current(page);
  51  |       expect(initial.authoring.palette.length).toBeGreaterThan(80);
  52  |       const basis = initial.authoring.checked_plot_id;
  53  |       const semanticGears = initial.face.subjects.filter(subject => subject.role === "Gear")
  54  |         .map(subject => initial.face.properties.find(property => property.subject === subject.identity
  55  |           && property.name === "semantic-id").value.Identity).sort();
  56  |       expect(initial.face.basis.plan_id).toBeNull();
  57  |       const mapping = initial.authoring.palette.find(entry => entry.kind_id === "math/map-quantity");
  58  |       expect(mapping.authorable).toBe(true);
  59  |       expect(mapping.kind_contract_revision).toBeTruthy();
  60  |       expect(mapping.configuration.length).toBeGreaterThan(4);
  61  |       expect(mapping.front).toBeTruthy();
  62  |       const distinctBases = await page.evaluate(async basis => {
  63  |         const { authoringBasis } = await import("/assets/authoring.js");
  64  |         return [
  65  |           authoringBasis({ ...basis, source_document_id: `${basis.source_document_id}/other` }),
  66  |           authoringBasis({ ...basis, checked_plot_id: `${basis.checked_plot_id}/other` }),
  67  |           authoringBasis({ ...basis, expanded_plot_id: `${basis.expanded_plot_id}/other` }),
  68  |           authoringBasis(basis),
  69  |         ];
  70  |       }, initial.authoring);
  71  |       expect(new Set(distinctBases).size).toBe(4);
  72  | 
  73  |       const portIdentity = (snapshot, gear, direction) => {
  74  |         const subject = snapshot.face.subjects.find(subject => subject.role === "Gear"
  75  |           && snapshot.face.properties.some(property => property.subject === subject.identity
  76  |             && property.name === "semantic-id" && property.value.Identity === `gear/authoring/${gear}`));
  77  |         const ports = snapshot.face.relationships.filter(relation => relation.kind === "Contains" && relation.source === subject.identity);
  78  |         const port = ports.find(relation => snapshot.face.properties.some(property =>
  79  |           property.subject === relation.target && property.name === "direction" && property.value.Text === direction));
  80  |         return snapshot.face.properties.find(property => property.subject === port.target && property.name === "semantic-id").value.Identity;
  81  |       };
  82  |       const queries = await page.evaluate(async ({ sourcePort, sinkPort }) => {
  83  |         const snapshot = await (await fetch("/api/snapshot")).json();
  84  |         const response = await fetch("/api/authoring-query", { method: "POST", headers: { "Content-Type": "application/json" },
  85  |           body: JSON.stringify({ query: "connections", revision: snapshot.authoring.source_revision, expanded_plot_id: snapshot.authoring.expanded_plot_id, source: sourcePort }) });
  86  |         return (await response.json()).candidates.find(candidate => candidate.sink_identity === sinkPort);
  87  |       }, { sourcePort: portIdentity(initial, "map", "outgoing"), sinkPort: portIdentity(initial, "quantity", "receiving") });
  88  |       expect(queries.compatible).toBe(false);
  89  |       expect(queries.diagnostic).toBeTruthy();
  90  |       expect(queries.adapters.map(adapter => adapter.kind_id)).toContain("structured-info/wrap-quantity");
  91  |       expect((await current(page)).authoring.checked_plot_id).toBe(basis);
  92  | 
  93  |       await selectRole(page, "Gear", "authoring/map Gear");
  94  |       const field = page.getByRole("textbox", { name: "Configure target-granularity", exact: true });
  95  |       await expect(field).toHaveValue(String(mapping.configuration.find(field => field.key === "target-granularity").default_value.I64));
  96  |       await expect(field).toHaveAccessibleDescription("Whole number from -9223372036854775808 to 9223372036854775807.");
  97  |       const configuration = page.locator('#authoring-actions [data-application-component="plot-field"]').filter({ hasText: "Configure target-granularity" });
  98  |       await page.route("**/api/authoring-query", route => route.abort("failed"));
  99  |       await configuration.getByRole("button", { name: "Apply", exact: true }).click();
  100 |       await expect(configuration.getByRole("status")).not.toBeEmpty();
  101 |       await expect(field).not.toHaveAttribute("aria-invalid");
  102 |       await page.unroute("**/api/authoring-query");
  103 |       await field.fill("9223372036854775808");
  104 |       await configuration.getByRole("button", { name: "Apply", exact: true }).click();
  105 |       await expect(configuration.getByRole("status")).not.toBeEmpty();
  106 |       await expect(field).toHaveAttribute("aria-invalid", "true");
  107 |       await expect(configuration.getByRole("button", { name: "Apply", exact: true }).locator('svg[aria-hidden="true"]')).toHaveCount(1);
```