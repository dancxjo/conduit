# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: sdk-external-body-execution.spec.mjs >> a Workspace Body continues through the browser SDK without a Patchbay page runtime
- Location: proof/browser/sdk-external-body-execution.spec.mjs:15:1

# Error details

```
Error: expect(locator).toHaveText(expected) failed

Locator: locator('#status')
Expected: "Browser Host participating"
Timeout: 15000ms
Error: element(s) not found

Call log:
  - Expect "toHaveText" with timeout 15000ms
  - waiting for locator('#status')

```

# Test source

```ts
  4   | import { join } from "node:path";
  5   | import { expect, test } from "@playwright/test";
  6   | import { startPresenceProbe } from "./browser-presence-support.mjs";
  7   | import { prepareFieldStationPackage } from "./field-station-package.mjs";
  8   | import { birthWorkspaceEvidence } from "./workspace-body-evidence-support.mjs";
  9   | 
  10  | test.beforeAll(async ({}, testInfo) => {
  11  |   testInfo.setTimeout(120_000);
  12  |   await prepareFieldStationPackage();
  13  | });
  14  | 
  15  | test("a Workspace Body continues through the browser SDK without a Patchbay page runtime", async ({ page }) => {
  16  |   const journey = await openExternalBody(page, {
  17  |     friendlyName: "SDK Workbench",
  18  |     titles: ["Button Across the Room", "Clock", "Desk Telegraph"],
  19  |     forms: [
  20  |       ["button-across-room", "forms/button-across-room/main.conduit"],
  21  |       ["clock", "forms/clock/main.conduit"],
  22  |       ["desk_telegraph", "forms/desk-telegraph/main.conduit"],
  23  |     ],
  24  |   });
  25  |   try {
  26  |     await page.evaluate(() => globalThis.__conduitSdkParticipation.begin());
  27  |     await expect(page.locator("#external-body-output output")).toHaveCount(3);
  28  |     await expect(page.locator("#external-body-output")).toContainText("3");
  29  |     await page.getByRole("group", { name: "External Body Play input", exact: true }).hover();
  30  |     await page.mouse.down();
  31  |     await expect(page.locator('[data-presentation-kind="presentation/indicator-state"]')).toHaveText("true");
  32  |     await page.mouse.up();
  33  |     await expect(page.locator('[data-presentation-kind="presentation/indicator-state"]')).toHaveText("false");
  34  |     await expect(page.locator('[data-presentation-kind="presentation/tick"]'))
  35  |       .toHaveText(/^\d+$/, { timeout: 15_000 });
  36  |     const { receipt, terminal } = await finishExternalBody(page, journey);
  37  |     expect(receipt).toMatchObject({
  38  |       schema: "conduit.tour/manifestation-receipt@3",
  39  |       active_play_id: journey.claim.play.active_play_id,
  40  |       disposition: "cancelled",
  41  |       timer_completions: 4,
  42  |       manifestation_completions: 6,
  43  |     });
  44  |     expect(terminal.body_planning.execution_claims.at(-1).phase.Terminal.disposition).toBe("cancelled");
  45  |     expect(receipt.kernel_signs).toMatchObject({
  46  |       host_id: journey.host.id,
  47  |       boot_id: journey.host.bootId,
  48  |       active_play_id: journey.claim.play.active_play_id,
  49  |     });
  50  |     expect(receipt.kernel_signs.placements.map(({ placement_id }) => placement_id).sort()).toEqual(
  51  |       journey.proposal.plan.forms.flatMap(({ plan }) => plan.fragments
  52  |         .flatMap(({ placements }) => placements.map(({ placement_id }) => placement_id))).sort(),
  53  |     );
  54  |   } finally {
  55  |     await journey.cleanup();
  56  |   }
  57  | });
  58  | 
  59  | test("Rosehip measurement processing and bounded history continue through the SDK", async ({ page }) => {
  60  |   const journey = await openExternalBody(page, {
  61  |     friendlyName: "Rosehip House",
  62  |     titles: ["Button Across the Room", "Little Seismograph"],
  63  |     forms: [
  64  |       ["button-across-room", "forms/button-across-room/main.conduit"],
  65  |       ["little-seismograph-display", "forms/little-seismograph/main.conduit"],
  66  |     ],
  67  |   });
  68  |   try {
  69  |     await page.evaluate(() => globalThis.__conduitSdkParticipation.begin());
  70  |     await expect(page.locator('[data-presentation-kind="presentation/measurement-plot"]'))
  71  |       .toHaveText("plot 1 samples · 0 omitted");
  72  |     await expect(page.locator('[data-presentation-kind="presentation/measurement-threshold"]'))
  73  |       .toHaveText("threshold Above · Some(RoseAbove)");
  74  |     const { receipt, terminal } = await finishExternalBody(page, journey);
  75  |     expect(receipt).toMatchObject({
  76  |       schema: "conduit.tour/manifestation-receipt@3",
  77  |       active_play_id: journey.claim.play.active_play_id,
  78  |       disposition: "cancelled",
  79  |     });
  80  |     expect(terminal.body_planning.body_id).toBe(journey.bodyId);
  81  |     expect(terminal.body_planning.execution_claims).toHaveLength(1);
  82  |   } finally {
  83  |     await journey.cleanup();
  84  |   }
  85  | });
  86  | 
  87  | async function openExternalBody(page, { friendlyName, titles, forms }) {
  88  |   const temporary = await mkdtemp(join(tmpdir(), "conduit-sdk-external-body-"));
  89  |   const processes = [];
  90  |   const cleanup = async () => {
  91  |     for (const process of processes) if (process.exitCode === null) process.kill("SIGTERM");
  92  |     await rm(temporary, { recursive: true, force: true });
  93  |   };
  94  |   try {
  95  |     const { current, evidencePath } = await birthWorkspaceEvidence(
  96  |       page, temporary, friendlyName, titles, "body.json",
  97  |     );
  98  |     const probe = await startPresenceProbe(["--body-evidence", evidencePath]);
  99  |     processes.push(probe.process);
  100 |     const coordinator = await startCoordinator(evidencePath, probe.url, forms);
  101 |     processes.push(coordinator.process);
  102 | 
  103 |     await page.goto(`/proof/browser/sdk-body-participation/?body=${encodeURIComponent(probe.url)}`);
> 104 |     await expect(page.locator("#status")).toHaveText("Browser Host participating", { timeout: 15_000 });
      |                                           ^ Error: expect(locator).toHaveText(expected) failed
  105 |     await expect.poll(() => page.evaluate(() => {
  106 |       const observed = globalThis.__conduitSdkParticipation?.evidence();
  107 |       return Boolean(observed?.biography && observed?.offer);
  108 |     })).toBe(true);
  109 | 
  110 |     const api = path => new URL(`/api/${path}`, coordinator.url).href;
  111 |     const observed = await page.evaluate(() => globalThis.__conduitSdkParticipation.evidence());
  112 |     await post(page, api("body-membership-evidence"), observed.biography);
  113 |     await post(page, api("body-host-offer-evidence"), observed.offer);
  114 |     const requirements = await get(page, api("body-planning-requirements"));
  115 |     const selection = await page.evaluate(kindIds => {
  116 |       const sdk = globalThis.__conduitSdkParticipation;
  117 |       const executable = new Set(sdk.executionCapabilities());
  118 |       const advertisement = sdk.participation.advertisement;
  119 |       const capabilities = kindIds.map(kind => advertisement.capabilities
  120 |         .find(offer => offer.kind_id === kind && executable.has(offer.capability_id)));
  121 |       if (capabilities.some(offer => !offer)) throw new Error("SDK Host cannot realize the active Body workset");
  122 |       const resourceClasses = new Set(capabilities
  123 |         .flatMap(offer => offer.resource_requirements.map(requirement => requirement.class_id)));
  124 |       return {
  125 |         capabilityIds: [...new Set(capabilities.map(offer => offer.capability_id))].sort(),
  126 |         resourcePoolIds: advertisement.resources
  127 |           .filter(offer => resourceClasses.has(offer.class_id)).map(offer => offer.pool_id).sort(),
  128 |       };
  129 |     }, requirements.kind_ids);
  130 |     await page.evaluate(requested =>
  131 |       globalThis.__conduitSdkParticipation.participation.requestOfferEvidence(requested), selection);
  132 |     await expect.poll(() => page.evaluate(() =>
  133 |       globalThis.__conduitSdkParticipation.evidence().offer?.stage)).toBe("Planning");
  134 |     const planningOffer = await page.evaluate(() => globalThis.__conduitSdkParticipation.evidence().offer);
  135 |     await post(page, api("body-host-planning-offer"), planningOffer);
  136 | 
  137 |     const proposal = await get(page, api("body-execution-proposal"));
  138 |     expect(proposal.plan.body_id).toBe(current.body_id);
  139 |     const observations = await page.evaluate(value =>
  140 |       globalThis.__conduitSdkParticipation.prepare(value), proposal);
  141 |     expect(observations.length).toBeGreaterThanOrEqual(2);
  142 |     const host = await page.evaluate(() => globalThis.__conduitSdkParticipation.host.current());
  143 |     const claimed = await post(page, api("body-execution"), {
  144 |       schema: "conduit.patchbay/body-execution-request@1",
  145 |       action: { kind: "Claim", plan_id: proposal.plan.plan_id, host_id: host.id, boot_id: host.bootId },
  146 |     });
  147 |     const claim = claimed.body_planning.execution_claims.at(-1);
  148 |     const started = await page.evaluate(identity =>
  149 |       globalThis.__conduitSdkParticipation.start(identity), claim.play);
  150 |     expect(started.play).toEqual(claim.play);
  151 |     await post(page, api("body-execution"), {
  152 |       schema: "conduit.patchbay/body-execution-request@1",
  153 |       action: { kind: "Started", play: claim.play, wake_at_start: started.wakeAtStart },
  154 |     });
  155 |     return { api, bodyId: current.body_id, claim, cleanup, host, proposal };
  156 |   } catch (error) {
  157 |     await cleanup();
  158 |     throw error;
  159 |   }
  160 | }
  161 | 
  162 | async function finishExternalBody(page, journey) {
  163 |   const receipt = await page.evaluate(() => globalThis.__conduitSdkParticipation.cancel());
  164 |   const terminal = await post(page, journey.api("body-execution"), {
  165 |     schema: "conduit.patchbay/body-execution-request@1",
  166 |     action: {
  167 |       kind: "Terminal",
  168 |       play: journey.claim.play,
  169 |       disposition: receipt.disposition,
  170 |       terminal_sign_id: receipt.terminal_sign_id,
  171 |     },
  172 |   });
  173 |   return { receipt, terminal };
  174 | }
  175 | 
  176 | async function startCoordinator(evidencePath, invitation, forms) {
  177 |   const process = spawn("target/debug/patchbay-html", [
  178 |     "--body-evidence", evidencePath,
  179 |     "--external-reader",
  180 |     "--body-invitation", invitation,
  181 |     ...forms.flatMap(([name, source]) => ["--form", name, source]),
  182 |   ], { cwd: new URL("../..", import.meta.url).pathname, stdio: ["ignore", "pipe", "pipe"] });
  183 |   let output = "";
  184 |   const url = await new Promise((resolve, reject) => {
  185 |     const timeout = setTimeout(() => reject(new Error(`Body coordinator did not become ready\n${output}`)), 10_000);
  186 |     const inspect = chunk => {
  187 |       output += chunk.toString();
  188 |       const match = output.match(/PATCHBAY_HTML_URL=(http:\/\/127\.0\.0\.1:\d+)/);
  189 |       if (match) { clearTimeout(timeout); resolve(match[1]); }
  190 |     };
  191 |     process.stdout.on("data", inspect);
  192 |     process.stderr.on("data", inspect);
  193 |     process.once("exit", code => { clearTimeout(timeout); reject(new Error(`Body coordinator exited (${code})\n${output}`)); });
  194 |   });
  195 |   return { process, url };
  196 | }
  197 | 
  198 | async function get(page, url) {
  199 |   const response = await page.request.get(url);
  200 |   if (!response.ok()) throw new Error(`${url} returned ${response.status()}: ${await response.text()}`);
  201 |   return response.json();
  202 | }
  203 | 
  204 | async function post(page, url, data) {
```