import { spawn } from "node:child_process";
import { expect, test } from "@playwright/test";
import { installB7Devices, picoUf2 } from "./b7-fixture.mjs";

let entrance;

async function startBook() {
  const child = spawn("target/debug/conduit-browser-host", ["--book", "--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error(`executable book was not ready\n${output}`)),
      10_000,
    );
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/CONDUIT_BROWSER_HOST_URL=(http:\/\/127\.0\.0\.1:\d+\/book\/)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`executable book exited (${code})\n${output}`));
    });
  });
  return { child, url };
}

async function openStep(page, index) {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  for (let current = 0; current < index; current += 1) {
    await page.getByRole("button", { name: "Next" }).click();
  }
  await expect(page.locator(".creche-progress")).toHaveText(new RegExp(`^Page ${index + 1} of \\d+$`));
}

test.beforeEach(async () => {
  entrance = await startBook();
});

test.afterEach(() => entrance?.child.kill());

test("Step 0 births one named Morse Network Body before introducing machinery", async ({ page }) => {
  await openStep(page, 0);
  await expect(page.getByRole("heading", { name: "Step 0 — Birth your Body" })).toBeVisible();
  await expect(page.locator(".masthead")).toContainText("crèche");
  await expect(page).toHaveTitle(/The Crèche$/);
  await expect(page.locator(".body-birth-runner")).toHaveCount(1);
  await expect(page.locator(".gear-inventory")).toHaveCount(0);
  const runner = page.locator(".body-birth-runner");
  await expect(runner.getByLabel("Initial program")).toHaveValue("morse-network@1");
  await expect(runner.getByLabel("Friendly Body name")).not.toHaveValue("");
  await runner.getByLabel("Friendly Body name").fill("patient firefly");
  await runner.getByRole("button", { name: "Birth Body" }).click();
  await expect(runner.locator(".body-state")).toHaveText("LULLED");
  await expect(runner.locator(".body-identities")).toContainText("patient firefly");
  await expect(page.locator(".creche-body-context")).toContainText("patient firefly");
  const raw = JSON.parse(await runner.locator(".body-raw code").textContent());
  expect(raw.membership.parts).toHaveLength(0);
  expect(raw.body.events).toHaveLength(1);
});

test("Steps 0 through 12 lead with human motivation and return to the Conduit payoff", async ({
  page,
}) => {
  const anchors = [
    "build one computer out of the computers you actually have",
    "useful programs will still evolve",
    "different finite machines",
    "vocabulary fragments",
    "should not have to copy its internal machinery",
    "constrained systems unnecessarily large",
    "minimal viable Conduit Host",
    "bounded and portable",
    "Machine-specific truth stays with the machine",
    "Look what just happened",
    "replaceable answer to current circumstances",
    "durable computer Conduit is maintaining",
    "A successful deployment is only deployment",
  ];
  await openStep(page, 0);
  for (let step = 0; step < anchors.length; step += 1) {
    await expect(page.locator("#chapter")).toContainText(anchors[step]);
    await expect(page.getByRole("heading", { name: "Conduit idea" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Payoff" })).toBeVisible();
    const firstParagraph = page.locator(".chapter-copy").first().locator("p").first();
    await expect(firstParagraph).toBeVisible();
    if (step < anchors.length - 1) await page.getByRole("button", { name: "Next" }).click();
  }
});

test("Steps 1 through 3 add substitution, explicit fan-out, and one generic verb", async ({ page }) => {
  await openStep(page, 1);
  const hostId = await page.evaluate(() => globalThis.__conduitBookHost.hostId);
  let runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("HELLO");
  await expect(runner.locator(".play-status")).toContainText("Completed");

  await page.getByRole("button", { name: "Next" }).click();
  await expect(page.getByRole("heading", { name: "Step 2 — Fan out explicitly" })).toBeVisible();
  runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("true");
  await expect(runner.locator(".play-status")).toContainText("Completed");

  await page.getByRole("button", { name: "Next" }).click();
  await expect(page.getByRole("heading", { name: "Step 3 — Use a generic verb" })).toBeVisible();
  runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("3.000000");
  await expect(runner.locator(".play-status")).toContainText("Completed");
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toBe(hostId);
});

test("Steps 4 through 6 reveal a Back and compare two realizations deliberately", async ({ page }) => {
  await openStep(page, 4);
  await expect(page.getByLabel("Morse realization")).toHaveCount(0);
  let runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("·");
  await expect(runner.locator(".play-status")).toContainText("Completed");
  await expect(runner.locator(".expansion")).toContainText("Selected realization: direct");
  await expect(runner.locator(".expansion")).not.toContainText("Opened reusable Forms");

  await page.getByRole("button", { name: "Next" }).click();
  runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("··· ——— ···");
  await expect(runner.locator(".expansion")).toContainText("Selected realization: direct");
  await expect(runner.locator(".expansion")).not.toContainText("Opened reusable Forms");

  await page.getByRole("button", { name: "Next" }).click();
  const comparison = page.locator(".realization-comparison");
  await expect(comparison.locator(".shared-face span")).toHaveText("Same requested Face");
  await expect(comparison.locator(".shared-face code")).toHaveText(
    "text/morse · text: value/text@1 → pattern: value/morse-pattern@1",
  );
  await expect(page.getByText("minimal viable Conduit Host", { exact: false })).toBeVisible();
  const direct = comparison.locator(".runner").nth(0);
  const recursive = comparison.locator(".runner").nth(1);
  await expect(direct.getByRole("heading", { name: "Direct leaf" })).toBeVisible();
  await expect(recursive.getByRole("heading", { name: "Recursive Form Back" })).toBeVisible();
  const listing = direct.locator("textarea");
  await listing.fill((await listing.inputValue()).replace('"HELLO"', '"E"'));
  await expect(recursive.locator("textarea")).toHaveValue(await listing.inputValue());
  await direct.getByRole("button", { name: "Run direct leaf" }).click();
  await expect(direct.locator(".play-status")).toContainText("Completed");
  await recursive.getByRole("button", { name: "Run recursive Back" }).click();
  await expect(recursive.locator(".play-status")).toContainText("Completed");
  await expect(direct.locator(".morse")).toHaveText("·");
  await expect(recursive.locator(".morse")).toHaveText("·");
  await expect(direct.locator(".expansion")).toContainText("Selected realization: direct");
  await expect(direct.locator(".expansion")).not.toContainText("Opened reusable Forms");
  await expect(recursive.locator(".expansion")).toContainText("Selected realization: recursive");
  await expect(recursive.locator(".expansion")).toContainText("Opened reusable Forms");
  await expect(recursive.locator(".expansion")).toContainText("morse/lookup");
  const directIdentities = await direct.locator("details dd").allTextContents();
  const recursiveIdentities = await recursive.locator("details dd").allTextContents();
  expect(directIdentities[0]).toBe(recursiveIdentities[0]);
  expect(directIdentities[1]).toBe(recursiveIdentities[1]);
  expect(directIdentities[2]).not.toBe(recursiveIdentities[2]);
  expect(directIdentities[3]).not.toBe(recursiveIdentities[3]);
});

test("Crèche navigation preserves drafts while reset and revisit change presentation only", async ({ page }) => {
  await openStep(page, 1);
  const hostId = await page.evaluate(() => globalThis.__conduitBookHost.hostId);
  const edited = (await page.locator("textarea").inputValue()).replace('"hello"', '"reader"');
  await page.locator("textarea").fill(edited);
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Previous" }).click();
  await expect(page.locator("textarea")).toHaveValue(edited);
  await page.getByRole("button", { name: "Reset this page" }).click();
  await expect(page.locator("textarea")).toHaveValue(/"hello"/);
  await page.getByRole("button", { name: "Revisit birth page" }).click();
  await expect(page.locator(".creche-progress")).toHaveText(/^Page 1 of \d+$/);
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toBe(hostId);
});

test("unsupported capability and type mismatch remain ordinary pre-Play refusals", async ({ page }) => {
  await openStep(page, 1);
  const runner = page.locator(".runner");
  const listing = runner.locator("textarea");
  await listing.fill(`form unavailable {
    source: text/literal("still planned")
    result: presentation/text
    missing: layout/inset
    source > result
  }`);
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".play-status")).toContainText(
    "refused before Play · missing-implementation-or-placement",
  );
  await listing.fill(`form wrong-type {
    source: scalar/literal(1.0)
    invert: logic/not
    result: presentation/bool-value
    source > invert > result
  }`);
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".play-status")).toContainText(
    "refused before Play · type-or-source",
  );
  await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
});

test("Step 7 presents startup and current count through four admitted browser ticks", async ({ page }) => {
  await openStep(page, 7);
  await expect(page.getByRole("heading", { name: "Step 7 — State over time" })).toBeVisible();
  const runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("0");
  await expect(runner.locator(".morse")).toHaveText("4");
  await expect(runner.locator(".play-status")).toContainText(
    "4 planned ticks, 5 presentations",
  );
  await expect(runner.locator(".play-status")).toHaveAttribute("data-timer-completions", "4");
  await expect(runner.locator(".play-status")).toHaveAttribute(
    "data-manifestation-completions",
    "5",
  );
  await expect(runner.locator("details dd")).toHaveCount(12);
});

test("stopping Step 7 cancels the pending timer without a late completion", async ({ page }) => {
  await openStep(page, 7);
  const runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("0");
  await runner.getByRole("button", { name: "Stop" }).click();
  await expect(runner.locator(".play-status")).toHaveText("Stopped. The Play was cancelled.");
  await page.waitForTimeout(650);
  await expect(runner.locator(".play-status")).toHaveText("Stopped. The Play was cancelled.");
  await expect(runner.locator(".play-status")).not.toHaveAttribute("data-receipt", /.+/);
  await expect(runner.locator(".morse")).toHaveText("0");
});

test("Step 8 shows the exact installed offers from the planning advertisement", async ({ page }) => {
  await openStep(page, 8);
  await expect(page.getByRole("heading", { name: "Step 8 — Meet the Host" })).toBeVisible();
  const inventory = page.locator(".gear-inventory");
  await expect(inventory).toHaveCount(1);
  const visibleInstalled = await inventory.locator("li.available code").allTextContents();
  const advertisedInstalled = await page.evaluate(() => {
    const api = globalThis.__conduitBookHost.runtime;
    api.conduit_book_inventory();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_book_output_ptr(), api.conduit_book_output_len());
    return JSON.parse(new TextDecoder().decode(bytes)).entries
      .filter((entry) => entry.implementation_id !== null)
      .map((entry) => entry.kind_id);
  });
  expect(visibleInstalled).toEqual(advertisedInstalled);
  expect(visibleInstalled).toEqual(expect.arrayContaining([
    "time/every", "state/count", "presentation/count", "logic/select",
    "layout/viewport", "time/delay", "input/keyboard", "presentation/bool",
  ]));
});

test("Step 9 executes one unchanged Form across two independent browser Hosts", async ({ page }) => {
  await openStep(page, 9);
  await expect(page.getByRole("heading", { name: "Step 9 — Two browser Hosts" })).toBeVisible();
  const runner = page.locator(".multi-host-runner");
  const source = await runner.locator("textarea").inputValue();
  expect(source).not.toMatch(/HostId|BootId|browser\/|iframe|DOM|socket|address/);
  await runner.getByRole("button", { name: "Run across two Hosts" }).click();
  await expect(runner.locator(".play-status")).toContainText(
    "one immutable Plan, two independent Plays, one delivered cross-Host value",
  );
  await expect(runner.locator(".morse")).toHaveText("hello across one planned Cord");
  await expect(runner.locator(".host-a strong")).toHaveText("completed");
  await expect(runner.locator(".host-b strong")).toHaveText("completed");
  const identities = await page.evaluate(() => ({
    a: globalThis.__conduitBookHost,
    b: globalThis.__conduitBookPeerHost,
  }));
  expect(identities.a.hostId).not.toBe(identities.b.hostId);
  expect(identities.a.bootId).not.toBe(identities.b.bootId);
  await expect(page.locator("iframe")).toHaveCount(0);
  await expect(runner.locator(".projected-cord")).toContainText("1 item");
  await expect(runner.locator(".play-status")).toHaveAttribute("data-source-receipt", /.+/);
  await expect(runner.locator(".play-status")).toHaveAttribute("data-sink-receipt", /.+/);
});

test("Step 10 compact and raw views project the same exact immutable Plan", async ({ page }) => {
  await openStep(page, 10);
  await expect(page.getByRole("heading", { name: "Step 10 — Plans and Plays" })).toBeVisible();
  const runner = page.locator(".multi-host-runner");
  await expect(runner.locator(".plan-view-details")).toHaveAttribute("open", "");
  await runner.getByRole("button", { name: "Run across two Hosts" }).click();
  await expect(runner.locator(".play-status")).toContainText("Completed");
  const projectedPlanId = await runner.locator(".projected-plan-id").textContent();
  const rawPlan = JSON.parse(await runner.locator(".raw-plan code").textContent());
  expect(rawPlan.plan_id).toBe(projectedPlanId);
  expect(rawPlan.fragments).toHaveLength(2);
  expect(new Set(rawPlan.fragments.map((fragment) => fragment.host_id)).size).toBe(2);
  await expect(runner.locator(".projected-hosts article")).toHaveCount(2);
  await expect(runner.locator(".projected-hosts")).toContainText("text/literal");
  await expect(runner.locator(".projected-hosts")).toContainText("presentation/text");
});

test("Step 0 explicitly births one LULLED Body and Step 1 admits its first Host", async ({ page }) => {
  await openStep(page, 0);
  await expect(page.getByRole("heading", { name: "Step 0 — Birth your Body" })).toBeVisible();
  let runner = page.locator(".body-birth-runner");
  const source = await runner.locator("textarea").inputValue();
  expect(source).not.toMatch(/HostId|BootId|browser\/|DOM|socket|address|Wake|Plan|Play/);
  await runner.getByRole("button", { name: "Birth Body" }).click();
  await expect(runner.locator(".birth-status")).toContainText(
    "one checked Seed now has one LULLED Body; no Wake, Plan, or Play exists",
  );
  await expect(runner.locator(".body-state")).toHaveText("LULLED");
  await expect(runner.getByRole("button", { name: "Birth Body" })).toBeDisabled();
  await expect(runner.locator(".body-identities")).toContainText("none yet");
  const identity = await runner.evaluate((element) => ({
    bodyId: element.dataset.bodyId,
    birthSignId: element.dataset.birthSignId,
  }));
  expect(identity.bodyId).toHaveLength(64);
  expect(identity.birthSignId).toHaveLength(64);
  const raw = JSON.parse(await runner.locator(".body-raw code").textContent());
  expect(raw.body.state).toBe("Lulled");
  expect(raw.body.events).toEqual([{ Born: { sign_id: identity.birthSignId } }]);
  expect(raw.membership.parts).toHaveLength(0);
  expect(raw.membership.events).toHaveLength(0);

  await page.getByRole("button", { name: "Next" }).click();
  await expect(page.locator(".creche-body-context")).toContainText(identity.bodyId);
  const firstHost = page.locator(".first-host-runner");
  await firstHost.getByRole("button", { name: "Give this Body its first Host" }).click();
  await expect(firstHost.locator(".host-admission-status")).toContainText("one admitted browser Host");
  await expect(firstHost.locator(".host-identities")).toContainText(identity.bodyId);
  await expect(page.locator(".creche-body-context")).toContainText("1 admitted Part");

  const expectSameBody = async () => {
    runner = page.locator(".body-birth-runner");
    await expect(runner).toHaveAttribute("data-body-id", identity.bodyId);
    await expect(runner).toHaveAttribute("data-birth-sign-id", identity.birthSignId);
    await expect(runner.locator(".birth-status")).toContainText("Same LULLED Body retained");
    await expect(runner.getByRole("button", { name: "Birth Body" })).toBeDisabled();
  };
  await page.getByRole("button", { name: "Previous" }).click();
  await expectSameBody();
  await page.getByRole("button", { name: "Reset this page" }).click();
  await expectSameBody();
  await page.getByRole("button", { name: "Revisit birth page" }).click();
  await expect(page.locator(".creche-progress")).toHaveText(/^Page 1 of \d+$/);
  await expectSameBody();
});

test("Step 12 keeps IMAGE, deployment, Boot, join, admission, offers, Plan, and Play distinct", async ({ page }) => {
  await installB7Devices(page);
  await openStep(page, 0);
  await page.getByRole("button", { name: "Birth Body" }).click();
  const birth = await page.locator(".body-birth-runner").evaluate((element) => ({
    bodyId: element.dataset.bodyId,
    birthSignId: element.dataset.birthSignId,
  }));
  for (let step = 0; step < 12; step += 1) await page.getByRole("button", { name: "Next" }).click();
  await expect(page.getByRole("heading", { name: "Step 12 — Add a physical Host" })).toBeVisible();
  const runner = page.locator(".physical-host-runner");
  await runner.locator("input[type=file]").setInputFiles({
    name: "reviewed-pico-local.uf2",
    mimeType: "application/octet-stream",
    buffer: Buffer.from(picoUf2()),
  });
  await expect(runner.locator('[data-stage="image"]')).toHaveClass(/complete/);
  await expect(runner.locator(".physical-status")).toContainText("No spore, deployment, Boot, or membership exists");

  await runner.getByRole("button", { name: "Prepare Body spore" }).click();
  await expect(runner.locator('[data-stage="spore"]')).toHaveClass(/complete/);
  await expect(runner.locator(".physical-status")).toContainText("Deployment, Boot, join, membership, offers, Plan, and Play remain absent");

  await runner.getByRole("button", { name: "Connect BOOTSEL and deploy" }).click();
  await expect(runner.locator('[data-stage="deploy"] span')).toHaveText("RebootRequested");
  await expect(runner.locator(".physical-status")).toContainText("That proves no Boot, join, membership, offers, readiness, Plan, or Play");

  await runner.getByRole("button", { name: "Connect running Pico and observe join" }).click();
  await expect(runner.locator('[data-stage="boot"]')).toHaveClass(/complete/);
  await expect(runner.locator(".physical-status")).toContainText("Admission remains an explicit action");

  await runner.getByRole("button", { name: "Admit physical Part" }).click();
  await expect(runner.locator('[data-stage="admit"]')).toHaveClass(/complete/);
  await expect(runner.locator(".physical-status")).toContainText("current offers are ready. No Plan or Play was created");
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.prepared.body_id).toBe(birth.bodyId);
  expect(evidence.prepared.invitation_secret).toBe("redacted");
  expect(evidence.deployment).toMatchObject({
    terminal: "RebootRequested",
    spore_id: evidence.prepared.spore_id,
    image_id: evidence.prepared.image_id,
    runtime_truth_created: false,
  });
  expect(evidence.observation.boot_id).toBe("pico-boot/b7-browser-proof");
  expect(evidence.admission).toMatchObject({
    disposition: "admitted",
    body_id: birth.bodyId,
    spore_id: evidence.prepared.spore_id,
    image_id: evidence.prepared.image_id,
    offers_observed: true,
    ready: true,
    plan_id: null,
    active_play_id: null,
  });
  expect(birth.birthSignId).toHaveLength(64);
});

test("Step 12 retains a refused WebUSB acquisition as terminal", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "usb", {
      configurable: true,
      value: {
        requestDevice: async () => {
          throw new DOMException("operator selected no BOOTSEL device", "NotFoundError");
        },
        addEventListener() {},
      },
    });
  });
  await openStep(page, 0);
  await page.getByRole("button", { name: "Birth Body" }).click();
  for (let step = 0; step < 12; step += 1) await page.getByRole("button", { name: "Next" }).click();
  const runner = page.locator(".physical-host-runner");
  await runner.locator("input[type=file]").setInputFiles({
    name: "reviewed-pico-local.uf2",
    mimeType: "application/octet-stream",
    buffer: Buffer.from(picoUf2()),
  });
  await runner.getByRole("button", { name: "Prepare Body spore" }).click();
  const deploy = runner.getByRole("button", { name: "Connect BOOTSEL and deploy" });
  await deploy.click();
  await expect(runner.locator(".physical-status")).toContainText("This USB acquisition is terminal");
  await expect(runner.locator('[data-stage="deploy"] span')).toHaveText("waiting");
  await expect(deploy).toBeDisabled();
});

test("Step 12 retains the exact Picoboot refusal chain", async ({ page }) => {
  await installB7Devices(page, { staleStatus: true });
  await openStep(page, 0);
  await page.getByRole("button", { name: "Birth Body" }).click();
  for (let step = 0; step < 12; step += 1) await page.getByRole("button", { name: "Next" }).click();
  const runner = page.locator(".physical-host-runner");
  await runner.locator("input[type=file]").setInputFiles({
    name: "reviewed-pico-local.uf2",
    mimeType: "application/octet-stream",
    buffer: Buffer.from(picoUf2()),
  });
  await runner.getByRole("button", { name: "Prepare Body spore" }).click();
  await runner.getByRole("button", { name: "Connect BOOTSEL and deploy" }).click();
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.deployment).toMatchObject({
    phase: "terminal",
    terminal: "StaleStatus",
    reboot_requested: false,
  });
  expect(evidence.deployment.failure_chain).toEqual([
    "StaleStatus: RP2040 deployment terminated without success",
    "StaleStatus: PICOBOOT status belongs to a different command identity",
  ]);
  await expect(runner.locator(".physical-status")).toContainText("StaleStatus");
});

test("stopping the two-Host lesson cancels without a late manifestation", async ({ page }) => {
  await page.addInitScript(() => {
    const callbacks = [];
    globalThis.requestAnimationFrame = (callback) => {
      callbacks.push(callback);
      return callbacks.length;
    };
    globalThis.__releaseBookAnimationFrame = () => {
      for (const callback of callbacks.splice(0)) callback(performance.now());
    };
  });
  await openStep(page, 9);
  const runner = page.locator(".multi-host-runner");
  await runner.getByRole("button", { name: "Run across two Hosts" }).click();
  await expect(runner.locator(".play-status")).toContainText("Host A offered one value");
  await runner.getByRole("button", { name: "Stop" }).click();
  await expect(runner.locator(".play-status")).toHaveText("Stopped. The Play was cancelled.");
  await page.evaluate(() => globalThis.__releaseBookAnimationFrame());
  await expect(runner.locator(".play-status")).toHaveText("Stopped. The Play was cancelled.");
  await expect(runner.locator(".morse")).toHaveText("ready");
  await expect(runner.locator(".play-status")).not.toHaveAttribute("data-source-receipt", /.+/);
});
