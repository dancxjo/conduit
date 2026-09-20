import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { randomUUID } from "node:crypto";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;
test.beforeEach(async () => { entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/"); });
test.afterEach(() => entrance?.child.kill());

test("the ordinary Face binds and admits one compiler-free reviewed browser Host", async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Firefly Choir", exact: true }).check();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "parts / hosts", exact: true }).click();
  await expect(page.getByLabel("parts and hosts").getByRole("button", { name: "Invite another phone", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Add a Host", exact: true }).click();
  await expect(page.getByRole("heading", { name: "What should this browser contribute?", exact: true })).toBeVisible();
  await expect(page.getByText("it creates no Host, membership, readiness, offer, Plan, or Play", { exact: false })).toBeVisible();
  await expect(page.getByText("Target: Browser page (browser/wasm32/page) · bind reviewed superset", { exact: true })).toBeVisible();
  await expect(page.getByRole("checkbox", { name: "Display this Body", exact: true })).toBeChecked();
  await expect(page.getByRole("checkbox", { name: "Keyboard and pointer input", exact: true })).toBeChecked();
  await page.getByRole("checkbox", { name: "Microphone input", exact: true }).check();
  await expect(page.getByRole("button", { name: "Continue with reviewed Host", exact: true })).toHaveCount(0);

  await page.getByRole("button", { name: "Resolve reviewed Bases", exact: true }).click();
  await expect(page.locator('[data-application-key="configuration-review-values"]')).toContainText("PROFILE");
  await expect(page.locator('[data-application-key="configuration-review-values"]')).toContainText("browser/media-devices-microphone@1");
  await page.getByRole("button", { name: "Back / Edit", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: /browser\/dom@1/ })).toBeChecked();
  await expect(page.getByRole("checkbox", { name: /browser\/media-devices-microphone@1/ })).toBeChecked();
  await page.getByRole("button", { name: "Review Host", exact: true }).click();
  await expect(page.getByRole("button", { name: "Continue with reviewed Host", exact: true })).toBeVisible();
  const before = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  const beforeRealization = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().realization);
  await page.getByRole("button", { name: "Continue with reviewed Host", exact: true }).click();
  const runner = page.locator(".physical-host-runner");
  await expect(page.getByRole("heading", { name: "Bind and admit this reviewed browser Host", exact: true })).toBeVisible();
  await expect(runner.locator('[data-application-key="physical-target"]')).toHaveValue("browser/wasm32/page");
  await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).not.toContainText("waiting");
  let evidence = JSON.parse((await runner.locator(".physical-evidence details code").allTextContents()).join(""));
  expect(evidence.obtainment).toMatchObject({
    target_id: "browser/wasm32/page",
    profile_id: expect.stringMatching(/^sha256:[0-9a-f]{64}$/),
    source_configuration_id: expect.stringMatching(/^sha256:[0-9a-f]{64}$/),
    distribution_id: "conduit.browser/reviewed-distribution@1",
    build_id: expect.stringMatching(/^build:sha256:/),
    image_id: expect.stringMatching(/^image:sha256:/),
    builder_adapter: "conduit-host-browser/bind-prebuilt@1",
    compiler_started: false,
  });
  expect(evidence.obtainment.image_content_digest).not.toBe(evidence.obtainment.distribution_sha256);

  await runner.getByRole("button", { name: "Bind Body invitation", exact: true }).click();
  await expect(runner.locator('[data-application-key="physical-stage-bind"]')).not.toContainText("waiting");
  await expect(runner.locator('[data-application-key="download-spore"]')).toContainText("Download ZIP");
  evidence = JSON.parse((await runner.locator(".physical-evidence details code").allTextContents()).join(""));
  expect(evidence.binding).toMatchObject({
    body_id: await page.evaluate(() => globalThis.__conduitWorkspace.current().body_id),
    browser_configuration_id: evidence.obtainment.source_configuration_id,
    browser_profile_id: evidence.obtainment.profile_id,
    image_content_digest: evidence.obtainment.image_content_digest,
  });
  expect(await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership)).toEqual(before);
  expect(await page.evaluate(() => globalThis.__conduitWorkspace.evidence().realization)).toEqual(beforeRealization);

  await page.evaluate(() => {
    let next = 1n;
    Object.defineProperty(Crypto.prototype, "randomUUID", {
      configurable: true,
      value: () => `00000000-0000-4000-8000-${(next++).toString(16).padStart(12, "0")}`,
    });
  });
  await runner.getByRole("button", { name: "Realize selected Host", exact: true }).click();
  await expect(runner.locator('[data-application-key="physical-stage-realize"] dd')).toHaveText("BrowserBundleLoaded");
  evidence = JSON.parse((await runner.locator(".physical-evidence details code").allTextContents()).join(""));
  const admittedImplementationIds = evidence.realization.implementation_registry?.map(({ id }) => id)
    ?? evidence.realization.retained_summary?.admitted_implementation_ids;
  const readyImplementationIds = evidence.realization.offers?.map(({ implementation_id }) => implementation_id)
    ?? evidence.realization.retained_summary?.ready_implementation_ids;
  expect(admittedImplementationIds.sort()).toEqual([
    "browser/dom-presentation@1",
    "browser/dom@1",
    "browser/keyboard-events@1",
    "browser/media-devices-microphone@1",
    "browser/pointer-events@1",
  ]);
  expect(readyImplementationIds.sort()).toEqual([
    "browser/dom-presentation@1",
    "browser/dom@1",
    "browser/keyboard-events@1",
    "browser/media-devices-microphone@1",
    "browser/pointer-events@1",
  ]);
  expect(await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership)).toEqual(before);
  await runner.getByRole("button", { name: "Observe Boot and join", exact: true }).click();
  const admit = runner.getByRole("button", { name: "Admit Part and offers", exact: true });
  await expect(admit).toBeEnabled({ timeout: 15_000 });
  expect(await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership)).toEqual(before);
  await admit.click();
  await expect(page.locator(".member-card")).toHaveCount(2);
  const after = await page.evaluate(() => globalThis.__conduitWorkspace.evidence());
  expect(after.evidence.membership.revision).toBe(before.revision + 2);
  expect(after.evidence.membership.parts.every(part => part.state === "Admitted" && part.current)).toBe(true);
  expect(after.current_host_offers).toHaveLength(2);
  const added = after.evidence.membership.parts.find(part => part.current.host_id !== before.parts[0].current.host_id);
  const offer = after.current_host_offers.find(candidate => candidate.host_id === added.current.host_id);
  expect(offer.boot_id).toBe(added.current.boot_id);
  expect(offer.host_id).toBe(added.current.host_id);
  expect(offer.capabilities.length).toBeGreaterThan(0);

  const authoredForms = await page.evaluate(() => globalThis.__conduitWorkspace.current().initial_forms);
  await page.locator("[data-close-membership]").click();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Refused");
  const replan = await page.evaluate(() => ({
    playback: globalThis.__conduitWorkspace.state(),
    evidence: globalThis.__conduitWorkspace.evidence(),
  }));
  expect(replan.playback.refusal).toMatchObject({
    code: "ExecutionLineUnavailable",
    message: "Body proposal selected an admitted Host without a current execution Line",
  });
  expect(replan.playback.proposal.plan.workset.forms).toEqual(
    authoredForms.map(({ source_document_id, checked_form_id }) => ({ source_document_id, checked_form_id })),
  );
  const plannedFragments = replan.playback.proposal.plan.forms.flatMap(({ plan }) => plan.fragments);
  expect(plannedFragments.map(({ host_id }) => host_id)).toEqual([added.current.host_id]);
  expect(plannedFragments.flatMap(({ placements }) => placements.map(({ implementation_id }) => implementation_id)))
    .toContain("browser/presentation-rhythm@1");
  const failedWake = replan.evidence.evidence.wakes.at(-1);
  expect(failedWake).toMatchObject({
    lifecycle: "Failed",
    plans: [{ plan_id: replan.playback.proposal.plan.plan_id, state: "AwaitingPlay" }],
    rejections: [{
      reason_code: "execution.line-unavailable",
      category: "Connectivity",
      required: 1,
      available: 0,
      plan_id: replan.playback.proposal.plan.plan_id,
    }],
  });
  expect(replan.evidence.evidence.body.workset.forms).toEqual(authoredForms);
  expect(replan.evidence.realization).toBeNull();

  await expect(page.locator('[data-application-key="tutorial-guidance"]')).toContainText("Tutorial · repair");
  await page.getByRole("button", { name: "Inspect lifecycle evidence", exact: true }).click();
  const inspected = JSON.parse(await page.locator("[data-inspection-content] pre").textContent());
  expect(inspected.refusal).toMatchObject({
    code: "ExecutionLineUnavailable",
    message: "Body proposal selected an admitted Host without a current execution Line",
  });
  expect(inspected.body.wakes.at(-1).rejections).toEqual(failedWake.rejections);
  await page.locator("[data-close-inspection]").click();

  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.getByRole("button", { name: "wake body", exact: true })).toBeVisible();
  const restored = await page.evaluate(() => ({
    current: globalThis.__conduitWorkspace.current(),
    evidence: globalThis.__conduitWorkspace.evidence(),
  }));
  expect(restored.current.body_id).toBe(replan.evidence.evidence.body.body_id);
  expect(restored.evidence.current_host_offers).toHaveLength(1);
  expect(restored.evidence.evidence.membership.parts.filter(part => !part.current)).toHaveLength(1);

  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const repaired = await page.evaluate(() => ({
    current: globalThis.__conduitWorkspace.current(),
    evidence: globalThis.__conduitWorkspace.evidence(),
  }));
  expect(repaired.current.body_id).toBe(restored.current.body_id);
  expect(repaired.evidence.realization.plan.plan_id).not.toBe(replan.playback.proposal.plan.plan_id);
  expect(repaired.evidence.evidence.wakes.at(-1).lifecycle).toBe("Playing");
  await expect(page.locator('[data-application-key="tutorial-guidance"]')).toContainText("Tutorial · continuity");
  await expect(page.locator('[data-application-key="tutorial-guidance"]')).not.toContainText("Tutorial · repair");
});

test("a second distinct browser Host explicitly joins through one canonical Body invitation", async ({ page, context, browser }) => {
  await page.addInitScript(() => Object.defineProperty(navigator, "share", {
    configurable: true,
    value: async value => { globalThis.__sharedInvitation = value; },
  }));
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Firefly Choir", exact: true }).check();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();

  await expect(page.getByRole("button", { name: "parts / hosts", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "parts / hosts", exact: true }).click();
  await expect(page.getByRole("heading", { name: "parts and hosts", exact: true })).toBeVisible();
  await expect(page.locator(".member-card")).toHaveCount(1);
  await expect(page.locator(".member-card")).toContainText("admitted · present · local Host");
  const before = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);

  await page.getByLabel("parts and hosts").getByRole("button", { name: "Invite another phone", exact: true }).click();
  await expect(page.locator('[data-application-key="invitation-status"]')).toContainText("Single-use Body invitation");
  const invitation = page.locator('[data-application-key="invitation-presentation"]');
  await expect(invitation.locator('[data-application-key="invitation-link"]')).toHaveCount(1);
  await expect(invitation).toHaveCSS("row-gap", "12px");
  const link = await invitation.locator('[data-application-key="invitation-link"]').textContent();
  const afterOffer = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  expect(afterOffer).toEqual(before);
  expect(new URL(link).hash).toContain("body-invitation=");
  await page.getByRole("button", { name: "Show QR", exact: true }).click();
  await expect(page.getByRole("img", { name: "QR representation of this exact Body invitation" })).toBeVisible();
  await page.getByRole("button", { name: "Share…", exact: true }).click();
  expect(await page.evaluate(() => globalThis.__sharedInvitation.url)).toBe(link);

  const receiverContext = await browser.newContext();
  const receiverPage = await receiverContext.newPage();
  await receiverPage.goto(entrance.url);
  await expect(receiverPage.getByLabel("Body invitation link or code", { exact: true })).toBeVisible();
  expect(await receiverPage.evaluate(() => globalThis.__conduitWorkspace.current())).toBeNull();
  const portableCode = new URLSearchParams(new URL(link).hash.slice(1)).get("body-invitation");
  await receiverPage.getByLabel("Body invitation link or code", { exact: true }).fill(portableCode);
  await receiverPage.getByRole("button", { name: "Inspect invitation", exact: true }).click();
  await expect(receiverPage.getByText("Body invitation", { exact: true })).toBeVisible();
  await expect(receiverPage.getByText("It grants no Form or effect authority.")).toBeVisible();
  expect(await receiverPage.evaluate(() => globalThis.__conduitWorkspace.current())).toBeNull();
  await receiverContext.close();

  const shareContext = await browser.newContext();
  const sharedPage = await shareContext.newPage();
  await sharedPage.goto(entrance.url);
  await sharedPage.evaluate(() => navigator.serviceWorker.ready);
  let shareDeliveryUrl = null;
  sharedPage.on("framenavigated", frame => {
    if (frame === sharedPage.mainFrame() && frame.url().includes("#body-share=")) shareDeliveryUrl = frame.url();
  });
  await sharedPage.evaluate(value => {
    const form = document.createElement("form");
    form.method = "post";
    form.action = new URL("share-body-invitation", location.href).href;
    const invitation = document.createElement("input");
    invitation.name = "body-invitation";
    invitation.value = value;
    form.append(invitation);
    document.body.append(form);
    form.submit();
  }, portableCode);
  await expect(sharedPage.getByText("Body invitation", { exact: true })).toBeVisible();
  await expect(sharedPage.getByText("It grants no Form or effect authority.")).toBeVisible();
  expect(await sharedPage.evaluate(() => ({ current: globalThis.__conduitWorkspace.current(), search: location.search }))).toEqual({ current: null, search: "" });
  expect(shareDeliveryUrl).toContain("#body-share=");
  const replayPage = await shareContext.newPage();
  await replayPage.goto(shareDeliveryUrl);
  await expect(replayPage.locator("[data-workspace-notice]")).toContainText("already consumed or unavailable");
  await shareContext.close();

  const joining = await context.newPage();
  await joining.goto(link);
  await joining.getByRole("button", { name: "Join this Body", exact: true }).click();

  await expect(joining.locator(".member-card")).toHaveCount(2);
  await expect(page.locator(".member-card")).toHaveCount(2);
  const authority = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  const receiver = await joining.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  expect(authority.revision).toBe(before.revision + 2);
  expect(receiver).toEqual(authority);
  expect(new Set(authority.parts.map(part => part.current.host_id)).size).toBe(2);
  expect(authority.parts.every(part => part.state === "Admitted" && part.current)).toBe(true);
  const authorityOffers = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers);
  const receiverOffers = await joining.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers);
  expect(authorityOffers).toHaveLength(2);
  expect(receiverOffers).toHaveLength(1);
  for (const offer of authorityOffers) {
    const member = authority.parts.find(part => part.current.host_id === offer.host_id);
    expect(offer.boot_id).toBe(member.current.boot_id);
    expect(offer.offer_generation).toBe(member.current.offer_generation);
  }

  await joining.close();
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await page.getByRole("button", { name: "parts / hosts", exact: true }).click();
  await expect(page.locator(".member-card")).toHaveCount(2);
  const restored = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);
  expect(restored.parts.filter(part => part.current)).toHaveLength(1);
  expect(restored.parts.filter(part => !part.current && part.state === "Admitted")).toHaveLength(1);
  const restoredOffers = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers);
  expect(restoredOffers).toHaveLength(1);
  expect(restoredOffers[0].host_id).toBe(restored.parts.find(part => part.current).current.host_id);
});

test("the POST share target refuses ambiguous, oversized, and GET delivery", async ({ page }) => {
  await page.goto(entrance.url);
  const manifest = await page.evaluate(async () => (await fetch(document.querySelector('link[rel="manifest"]').href)).json());
  expect(manifest.share_target).toEqual({
    action: "share-body-invitation",
    method: "POST",
    enctype: "application/x-www-form-urlencoded",
    params: { text: "body-invitation" },
  });
  await page.evaluate(() => navigator.serviceWorker.ready);
  const submit = values => page.evaluate(shared => {
    const form = document.createElement("form");
    form.method = "post";
    form.action = new URL("share-body-invitation", location.href).href;
    for (const value of shared) {
      const invitation = document.createElement("input");
      invitation.name = "body-invitation";
      invitation.value = value;
      form.append(invitation);
    }
    document.body.append(form);
    form.submit();
  }, values);

  await submit(["first", "second"]);
  await expect(page.locator("body")).toHaveText("Body invitation share is absent or duplicated");
  expect(new URL(page.url()).search).toBe("");

  await page.goto(entrance.url);
  await page.evaluate(() => navigator.serviceWorker.ready);
  await submit(["x".repeat(8193)]);
  await expect(page.locator("body")).toHaveText("Body invitation share exceeds its bound");

  const response = await page.goto(new URL("share-body-invitation?body-invitation=dummy", entrance.url).href);
  expect(response.status()).toBe(404);
});

test("pending share delivery survives worker replacement and expired delivery refuses", async ({ context, page }) => {
  await page.goto(entrance.url);
  await page.evaluate(() => navigator.serviceWorker.ready);
  const retain = ({ token, value, expiresAt }) => page.evaluate(({ token: identity, value: payload, expiresAt: expiry }) => new Promise((resolve, reject) => {
    const request = indexedDB.open("conduit-workspace-share-target", 1);
    request.onerror = () => reject(request.error);
    request.onupgradeneeded = () => request.result.createObjectStore("pending-invitations", { keyPath: "token" });
    request.onsuccess = () => {
      const transaction = request.result.transaction("pending-invitations", "readwrite");
      transaction.objectStore("pending-invitations").put({ token: identity, value: payload, expires_at_millis: expiry });
      transaction.oncomplete = () => { request.result.close(); resolve(); };
      transaction.onerror = () => reject(transaction.error);
    };
  }), { token, value, expiresAt });

  const restartToken = crypto.randomUUID();
  await retain({ token: restartToken, value: "not-json", expiresAt: Date.now() + 60_000 });
  await page.evaluate(async () => (await navigator.serviceWorker.ready).unregister());
  await page.close();
  const restarted = await context.newPage();
  await restarted.goto(`${entrance.url}#body-share=${restartToken}`);
  await expect(restarted.locator("[data-workspace-notice]")).toContainText("Body invitation is malformed");

  const expiredToken = crypto.randomUUID();
  await restarted.evaluate(({ token, expiresAt }) => new Promise((resolve, reject) => {
    const request = indexedDB.open("conduit-workspace-share-target", 1);
    request.onerror = () => reject(request.error);
    request.onupgradeneeded = () => request.result.createObjectStore("pending-invitations", { keyPath: "token" });
    request.onsuccess = () => {
      const transaction = request.result.transaction("pending-invitations", "readwrite");
      transaction.objectStore("pending-invitations").put({ token, value: "not-json", expires_at_millis: expiresAt });
      transaction.oncomplete = () => { request.result.close(); resolve(); };
      transaction.onerror = () => reject(transaction.error);
    };
  }), { token: expiredToken, expiresAt: Date.now() - 1 });
  await restarted.goto("about:blank");
  await restarted.goto(`${entrance.url}#body-share=${expiredToken}`);
  await expect(restarted.locator("[data-workspace-notice]")).toContainText("Body invitation share expired");
});

test("malformed invitation framing is refused without creating a Body", async ({ page }) => {
  await page.goto(`${entrance.url}#body-invitation=not-json`);
  await expect(page.locator("[data-workspace-notice]")).toContainText("Body invitation is malformed");
  expect(await page.evaluate(() => globalThis.__conduitWorkspace)).toBeUndefined();
});

test("an unavailable running Host leaves the current Body and browser Host intact", async ({ page }) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  const before = await page.evaluate(() => globalThis.__conduitWorkspace.evidence());

  await page.getByRole("button", { name: "parts / hosts", exact: true }).click();
  await page.getByRole("button", { name: "Connect a running Host", exact: true }).click();
  await page.getByLabel("Running Host rendezvous code", { exact: true }).fill("not-a-rendezvous-code");
  await page.getByRole("button", { name: "Connect Host", exact: true }).click();

  await expect(page.locator("[data-workspace-notice]")).toContainText("not a supported finite Line code");
  expect(await page.evaluate(() => globalThis.__conduitWorkspace.evidence())).toEqual(before);
  await page.getByRole("button", { name: "Back to members", exact: true }).click();
  await expect(page.locator(".member-card")).toHaveCount(1);
  await expect(page.locator(".member-card")).toContainText("admitted · present · local Host");
});

test("an already-running Host joins without displacing the browser Host or its Body", async ({ page }) => {
  const installed = await startInstalledHost();
  const running = spawn("target/debug/conduit", ["host", "rendezvous", "--state-dir", installed.stateDir, "--timeout-seconds", "30"], {
    cwd: installed.root,
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const code = await rendezvousCode(running);
    await page.goto(entrance.url);
    await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
    await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
    await page.getByRole("button", { name: "Birth Body", exact: true }).click();
    const before = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.membership);

    await page.getByRole("button", { name: "parts / hosts", exact: true }).click();
    await page.getByRole("button", { name: "Connect a running Host", exact: true }).click();
    await page.getByLabel("Running Host rendezvous code", { exact: true }).fill(code);
    await page.getByRole("button", { name: "Connect Host", exact: true }).click();

    await expect(page.locator(".member-card")).toHaveCount(2);
    await expect(page.locator(".member-card")).toContainText(["admitted · present · local Host", "admitted · present"]);
    const after = await page.evaluate(() => globalThis.__conduitWorkspace.evidence());
    expect(after.evidence.membership.revision).toBe(before.revision + 2);
    expect(after.evidence.membership.parts.every(part => part.state === "Admitted" && part.current)).toBe(true);
    expect(after.current_host_offers).toHaveLength(2);
    const nativeInstallation = JSON.parse(await readFile(`${installed.stateDir}/installation.json`, "utf8"));
    expect(nativeInstallation.joined_body_state?.body_id).toBe(after.evidence.body_id);
    const nativeRuntime = JSON.parse(await readFile(`${installed.stateDir}/runtime.json`, "utf8"));
    expect(nativeRuntime.body_id).toBe(after.evidence.body_id);
    await page.reload();
    await expectProcessSuccess(running);
    await page.getByRole("button", { name: "parts / hosts", exact: true }).click();
    await expect(page.locator(".member-card")).toHaveCount(2);
    const restored = await page.evaluate(() => globalThis.__conduitWorkspace.evidence());
    expect(restored.evidence.membership.parts.filter(part => part.current)).toHaveLength(1);
    expect(restored.current_host_offers).toHaveLength(1);
  } finally {
    if (running.exitCode === null) running.kill();
    await installed.close();
  }
});

test("loss of a joined Host Line removes only its current offers and keeps the Body", async ({ page }) => {
  const installed = await startInstalledHost();
  const running = spawn("target/debug/conduit", ["host", "rendezvous", "--state-dir", installed.stateDir, "--timeout-seconds", "30"], {
    cwd: installed.root,
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    const code = await rendezvousCode(running);
    await page.goto(entrance.url);
    await page.getByRole("checkbox", { name: "Memory Lantern", exact: true }).uncheck();
    await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
    await page.getByRole("button", { name: "Birth Body", exact: true }).click();
    const bodyId = await page.evaluate(() => globalThis.__conduitWorkspace.evidence().evidence.body_id);

    await page.getByRole("button", { name: "parts / hosts", exact: true }).click();
    await page.getByRole("button", { name: "Connect a running Host", exact: true }).click();
    await page.getByLabel("Running Host rendezvous code", { exact: true }).fill(code);
    await page.getByRole("button", { name: "Connect Host", exact: true }).click();
    await expect(page.locator(".member-card")).toHaveCount(2);
    expect(await page.evaluate(() => globalThis.__conduitWorkspace.evidence().current_host_offers.length)).toBe(2);

    running.kill();
    await expect(page.locator("[data-workspace-notice]")).toContainText("Voice stopped; Body Chat remains available");
    const after = await page.evaluate(() => globalThis.__conduitWorkspace.evidence());
    expect(after.evidence.body_id).toBe(bodyId);
    expect(after.evidence.membership.parts.filter(part => part.current)).toHaveLength(1);
    expect(after.evidence.membership.parts.filter(part => !part.current && part.state === "Admitted")).toHaveLength(1);
    expect(after.current_host_offers).toHaveLength(1);
  } finally {
    if (running.exitCode === null) running.kill();
    await installed.close();
  }
});

async function startInstalledHost() {
  const root = new URL("../..", import.meta.url).pathname;
  const stateDir = await mkdtemp(`${tmpdir()}/conduit-workspace-host-`);
  const productExecutable = `${stateDir}/reviewed-host-image`;
  await writeFile(productExecutable, "conduit workspace rendezvous proof image");
  await writeFile(`${stateDir}/installation.json`, JSON.stringify({
    schema: "conduit.install/durable-host@1",
    host_id: `host/workspace-proof/${randomUUID()}`,
    release_source_identity: "commit:workspace-proof",
    release_bundle_sha256: `sha256:${"a".repeat(64)}`,
    product_executable: productExecutable,
    body_state: null,
    joined_body_state: null,
  }));
  await writeFile(`${stateDir}/control.token`, Buffer.alloc(32, 7));
  const service = spawn("target/debug/conduit", ["host", "service", "run", "--state-dir", stateDir], {
    cwd: root,
    stdio: ["ignore", "pipe", "pipe"],
  });
  await processOutput(service, / is running/, "durable Host service");
  return { root, stateDir, close: async () => {
    if (service.exitCode === null) service.kill();
    await new Promise(resolveClose => service.exitCode === null ? service.once("exit", resolveClose) : resolveClose());
    await rm(stateDir, { recursive: true, force: true });
  } };
}

function rendezvousCode(child) {
  return processOutput(child, /Rendezvous code: (C1-WS-[0-9A-F-]+)/, "rendezvous code").then(match => match[1]);
}

function processOutput(child, pattern, label) {
  let output = "";
  return new Promise((resolveOutput, reject) => {
    const timeout = setTimeout(() => reject(new Error(`${label} was not ready\n${output}`)), 10_000);
    const inspect = chunk => {
      output += chunk.toString();
      const match = output.match(pattern);
      if (match) { clearTimeout(timeout); resolveOutput(match); }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", status => { clearTimeout(timeout); reject(new Error(`${label} exited before readiness (${status})\n${output}`)); });
  });
}

function expectProcessSuccess(child) {
  if (child.exitCode !== null) return Promise.resolve(expect(child.exitCode).toBe(0));
  return new Promise((resolve, reject) => child.once("exit", status => {
    try { expect(status).toBe(0); resolve(); }
    catch (error) { reject(error); }
  }));
}
