# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: browser-application-package.spec.mjs >> Crèche compatibility entrance redirects to the Workspace route
- Location: proof/browser/browser-application-package.spec.mjs:100:1

# Error details

```
Error: expect(received).toBe(expected) // Object.is equality

Expected: "/conduit/workspace/"
Received: "/conduit/creche/"

Call Log:
- Timeout 5000ms exceeded while waiting on the predicate
```

# Test source

```ts
  2   | import { openTourStep, startTour, startStaticProduct } from "./tour-test-server.mjs";
  3   | 
  4   | let entrance;
  5   | 
  6   | test.beforeEach(async ({}, testInfo) => {
  7   |   const creche = testInfo.title.startsWith("Crèche compatibility ");
  8   |   entrance = creche ? await startStaticProduct("target/creche-product", "/conduit/creche/") : await startTour();
  9   | });
  10  | test.afterEach(() => entrance?.child.kill());
  11  | 
  12  | async function mutatePackage(page, mutate) {
  13  |   await page.route("**/tour.application.json", async (route) => {
  14  |     const response = await route.fetch();
  15  |     const manifest = await response.json();
  16  |     mutate(manifest);
  17  |     await route.fulfill({ response, contentType: "application/json", body: JSON.stringify(manifest) });
  18  |   }, { times: 1 });
  19  | }
  20  | 
  21  | test("Tour drafts and an open reviewed Back endure a same-browser reload", async ({ page }) => {
  22  |   entrance.child.kill();
  23  |   entrance = await startStaticProduct("target/tour-product", "/conduit/tour/");
  24  |   const requests = [];
  25  |   page.on("request", (request) => {
  26  |     if (request.url().startsWith("http:")) requests.push(new URL(request.url()).pathname);
  27  |   });
  28  |   await page.goto(`${entrance.url}fronts-backs-and-implementation/`);
  29  |   await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  30  |   await expect(page.getByRole("heading", { name: "Fronts, Backs, and implementation" })).toBeVisible();
  31  |   const admission = await page.evaluate(() => ({
  32  |     applicationId: globalThis.__conduitBrowserApplication.manifest.applicationId,
  33  |     packageDigest: globalThis.__conduitBrowserApplication.manifest.packageDigest,
  34  |     stateIdentity: globalThis.__conduitBrowserApplication.manifest.stateCompatibility.identity,
  35  |     storageIdentity: globalThis.__conduitBrowserApplication.storage.applicationIdentity,
  36  |     storagePackageDigest: globalThis.__conduitBrowserApplication.storage.packageDigest,
  37  |     paths: globalThis.__conduitBrowserApplication.manifest.resources.map((resource) => resource.path),
  38  |     resourceUrls: globalThis.__conduitBrowserApplication.manifest.resources.map((resource) => resource.url.href),
  39  |   }));
  40  |   expect(admission.packageDigest).toMatch(/^sha256:[0-9a-f]{64}$/);
  41  |   expect(admission.applicationId).toBe("conduit.application/tour");
  42  |   expect(admission.stateIdentity).toBe("conduit.application/tour-reading-state");
  43  |   expect(admission.storagePackageDigest).toBe(admission.packageDigest);
  44  |   expect(admission.storageIdentity).toBe(admission.stateIdentity);
  45  |   const membershipClient = await page.evaluate(() => {
  46  |     const host = globalThis.__conduitTourHost;
  47  |     return {
  48  |       schema: host.membership.schema,
  49  |       hostId: host.hostId,
  50  |       bootId: host.bootId,
  51  |       advertisement: host.membership.advertisement(),
  52  |     };
  53  |   });
  54  |   expect(membershipClient.schema).toBe("conduit.browser/body-membership-client@1");
  55  |   expect(membershipClient.advertisement.host_id).toBe(membershipClient.hostId);
  56  |   expect(membershipClient.advertisement.boot_id).toBe(membershipClient.bootId);
  57  |   expect(admission.paths).toContain("tour-runner-presentation.mjs");
  58  |   expect(admission.paths).toContain("application-syntax-presentation.mjs");
  59  |   expect(admission.paths).toContain("assets/flow.css");
  60  |   for (const [index, path] of admission.paths.entries()) {
  61  |     const pathname = new URL(admission.resourceUrls[index]).pathname;
  62  |     expect(requests.filter((request) => request === pathname), path).toHaveLength(1);
  63  |   }
  64  |   await expect(page.locator('script[data-application-resource="react"]')).toHaveAttribute("src", /^blob:/);
  65  |   await expect(page.locator('style[data-application-resource="tour-style"]')).toHaveCount(1);
  66  |   await expect(page.locator('style[data-application-resource="patchbay-flow-style"]')).toHaveCount(1);
  67  |   const separatedStyles = await page.evaluate(() => {
  68  |     const application = globalThis.__conduitBrowserApplication;
  69  |     const decode = (role) => new TextDecoder().decode(application.bytes(role));
  70  |     return { tour: decode("tour-style"), flow: decode("patchbay-flow-style") };
  71  |   });
  72  |   expect(separatedStyles.tour).not.toContain(".flow-frontplate header");
  73  |   expect(separatedStyles.tour).not.toContain(".react-flow__edge-path");
  74  |   expect(separatedStyles.flow).toContain(".flow-frontplate header");
  75  |   expect(separatedStyles.flow).toContain(".react-flow__edge.animated");
  76  |   const runnerStatus = page.locator('[data-application-key="play-status"]');
  77  |   await expect(runnerStatus).toHaveAttribute("data-application-component", "status");
  78  |   await expect(runnerStatus).toHaveText("Edit the message or timing, then run it.");
  79  |   await expect(page.locator(".play-status")).toHaveCount(0);
  80  |   const listing = page.locator("textarea");
  81  |   const edited = (await listing.inputValue()).replace('"HELLO"', '"DURABLE"');
  82  |   await listing.fill(edited);
  83  |   const patchbay = page.locator(".compact-patchbay");
  84  |   await patchbay.getByRole("button", { name: "Open reviewed Back for same-morse-caller/morse" }).click();
  85  |   await expect(patchbay.locator(".gear-back-expansion")).toBeVisible();
  86  |   await page.evaluate(() => globalThis.__conduitTourPersistence.flush());
  87  | 
  88  |   await page.reload();
  89  |   await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  90  |   await expect(page.getByRole("heading", { name: "Fronts, Backs, and implementation" })).toBeVisible();
  91  |   await expect(page.locator("textarea")).toHaveValue(edited);
  92  |   await expect(page.locator(".gear-back-expansion")).toBeVisible();
  93  |   await expect(page.locator(".gear-back-flow")).toHaveAttribute("data-renderer", "react-flow");
  94  |   for (const [index, path] of admission.paths.entries()) {
  95  |     const pathname = new URL(admission.resourceUrls[index]).pathname;
  96  |     expect(requests.filter((request) => request === pathname), path).toHaveLength(2);
  97  |   }
  98  | });
  99  | 
  100 | test("Crèche compatibility entrance redirects to the Workspace route", async ({ page }) => {
  101 |   await page.goto(entrance.url);
> 102 |   await expect.poll(() => new URL(page.url()).pathname).toBe("/conduit/workspace/");
      |                                                         ^ Error: expect(received).toBe(expected) // Object.is equality
  103 | });
  104 | 
  105 | test("Tour navigation is one finite Host-manifested view with stale and pressure refusal", async ({ page }) => {
  106 |   await page.goto(entrance.url);
  107 |   await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  108 |   const navigation = page.locator('[data-application-slot="tour-navigation"]');
  109 |   await expect(navigation.locator('[data-application-component="navigation"]')).toHaveCount(1);
  110 |   await expect(navigation.locator('[data-application-key="progress"]')).toHaveText("Page 1 of 7");
  111 |   await expect(navigation.getByRole("button", { name: "Previous" })).toBeDisabled();
  112 |   await expect(navigation.getByRole("button", { name: "Next" })).toBeEnabled();
  113 | 
  114 |   await page.evaluate(() => {
  115 |     globalThis.__staleTourNavigationButton = document.querySelector('[data-application-key="next"]');
  116 |     globalThis.__staleTourNavigationButton.click();
  117 |   });
  118 |   await expect(page.getByRole("heading", { name: "Fronts, Backs, and implementation" })).toBeVisible();
  119 |   await page.evaluate(() => globalThis.__staleTourNavigationButton.click());
  120 |   expect(await page.evaluate(() => globalThis.__conduitBrowserApplication.presentation.lastRefusal("tour-navigation"))).toBe("stale-revision");
  121 |   await expect(page.getByRole("heading", { name: "Fronts, Backs, and implementation" })).toBeVisible();
  122 | 
  123 |   const presentationEvidence = await page.evaluate(() => {
  124 |     const presentation = globalThis.__conduitBrowserApplication.presentation;
  125 |     const before = document.querySelector('[data-application-slot="tour-navigation"]').innerHTML;
  126 |     let malformedRefusal;
  127 |     try {
  128 |       presentation.present("tour-navigation", {
  129 |         revision: 999,
  130 |         actions: [],
  131 |         nodes: Array.from({ length: 129 }, (_, index) => ({
  132 |           parent: index === 0 ? null : 0,
  133 |           component: "paragraph",
  134 |           key: `oversized-${index}`,
  135 |           text: "refuse before mutation",
  136 |           action: null,
  137 |         })),
  138 |       });
  139 |     } catch (error) { malformedRefusal = error.code; }
  140 |     const unchangedAfterRefusal = document.querySelector('[data-application-slot="tour-navigation"]').innerHTML === before;
  141 |     presentation.present("tour-navigation", {
  142 |       revision: 1_000,
  143 |       actions: [{ id: "proof.activate", event: "activate" }],
  144 |       nodes: [
  145 |         { parent: null, component: "navigation", key: "proof-nav", text: "Proof navigation", action: null },
  146 |         { parent: 0, component: "button", key: "proof-button", text: "Pressure", action: 0 },
  147 |       ],
  148 |     }, { eventCapacity: 1 });
  149 |     const button = document.querySelector('[data-application-key="proof-button"]');
  150 |     button.click();
  151 |     button.click();
  152 |     const event = presentation.nextEvent("tour-navigation");
  153 |     return {
  154 |       malformedRefusal,
  155 |       unchangedAfterRefusal,
  156 |       pressureRefusal: presentation.lastRefusal("tour-navigation"),
  157 |       event: { action: event.action, revision: event.revision, encodedBytes: event.encoded.length },
  158 |     };
  159 |   });
  160 |   expect(presentationEvidence).toEqual({
  161 |     malformedRefusal: "too-many-nodes",
  162 |     unchangedAfterRefusal: true,
  163 |     pressureRefusal: "queue-pressure",
  164 |     event: { action: "proof.activate", revision: 1_000, encodedBytes: 25 },
  165 |   });
  166 | });
  167 | 
  168 | test("browser Host refuses malformed and escaping application packages before launch", async ({ page }) => {
  169 |   for (const [mutate, refusal] of [
  170 |     [(manifest) => { manifest.schema = "wrong"; }, "browser application package schema is unsupported"],
  171 |     [(manifest) => { manifest.resources[0].path = "https://example.com/tour.mjs"; }, "application resource path escapes the application package"],
  172 |     [(manifest) => { manifest.resources.push(...Array.from({ length: 100 }, () => manifest.resources.at(-1))); }, "application package resource count is outside its admitted bound"],
  173 |   ]) {
  174 |     await mutatePackage(page, mutate);
  175 |     await page.goto(entrance.url);
  176 |     await expect(page.locator("#host-state")).toHaveText("Browser application refused");
  177 |     await expect(page.locator("#chapter")).toHaveText(refusal);
  178 |   }
  179 | });
  180 | 
  181 | test("browser Host refuses changed resource bytes before launch", async ({ page }) => {
  182 |   await page.route("**/tour.mjs", async (route) => {
  183 |     const response = await route.fetch();
  184 |     await route.fulfill({ response, body: `${await response.text()}\n// changed after packaging` });
  185 |   }, { times: 1 });
  186 |   await page.goto(entrance.url);
  187 |   await expect(page.locator("#host-state")).toHaveText("Browser application refused");
  188 |   await expect(page.locator("#chapter")).toHaveText("application resource application-module changed identity");
  189 |   expect(await page.evaluate(() => globalThis.__conduitTourHost)).toBeUndefined();
  190 | });
  191 | 
  192 | test("browser Host refuses a changed aggregate package identity before launch", async ({ page }) => {
  193 |   const resourceUrls = new Set();
  194 |   const requestedResources = [];
  195 |   await page.route("**/*", async (route) => {
  196 |     if (resourceUrls.has(route.request().url())) {
  197 |       requestedResources.push(route.request().url());
  198 |       await route.abort();
  199 |     } else {
  200 |       await route.fallback();
  201 |     }
  202 |   });
```