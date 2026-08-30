import { spawn } from "node:child_process";
import { expect, test } from "@playwright/test";
import { installB7Devices } from "./b7-fixture.mjs";

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

async function startCreche() {
  const child = spawn("target/debug/conduit-browser-host", ["--creche", "--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Crèche was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/CONDUIT_BROWSER_HOST_URL=(http:\/\/127\.0\.0\.1:\d+\/creche\/)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => { clearTimeout(timeout); reject(new Error(`Crèche exited (${code})\n${output}`)); });
  });
  return { child, url };
}

async function openStep(page, index) {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  for (let current = 0; current < index; current += 1) {
    await page.getByRole("button", { name: "Next" }).click();
  }
  await expect(page.locator(".book-progress")).toHaveText(new RegExp(`^Page ${index + 1} of \\d+$`));
}

async function openStandaloneCreche(page) {
  entrance.child.kill();
  entrance = await startCreche();
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
}

async function birthStandaloneBody(page, { attachFirstHost = false, sourceVariant = null } = {}) {
  await openStandaloneCreche(page);
  const birth = page.locator(".body-birth-runner");
  if (sourceVariant) {
    await birth.locator(".seed-source summary").click();
    const source = birth.locator("textarea");
    await source.fill((await source.inputValue()).replace('"SOS"', `"SOS ${sourceVariant}"`));
  }
  await birth.getByRole("button", { name: "Birth Body" }).click();
  const identity = await birth.evaluate((element) => ({
    bodyId: element.dataset.bodyId,
    birthSignId: element.dataset.birthSignId,
  }));
  if (attachFirstHost) {
    await page.getByRole("button", { name: "2. First Host" }).click();
    await page.getByRole("button", { name: "Give this Body its first Host" }).click();
  }
  return identity;
}

test.beforeEach(async () => {
  entrance = await startBook();
});

test.afterEach(() => entrance?.child.kill());

test("the Book opens as readable documentation and hands birth to the independent Crèche", async ({ page }) => {
  const responses = [];
  page.on("response", (response) => responses.push(new URL(response.url()).pathname));
  await openStep(page, 0);
  await expect(page.getByRole("heading", { name: "Bodies begin somewhere" })).toBeVisible();
  await expect(page).toHaveTitle(/The Book$/);
  await expect(page.locator(".body-birth-runner, .first-host-runner, .physical-host-runner, .graduation-runner")).toHaveCount(0);
  await expect(page.locator(".gear-inventory")).toHaveCount(0);
  const handoff = page.getByRole("link", { name: "Birth a Body" });
  await expect(handoff).toHaveAttribute("href", "../creche/");
  expect(responses.some((path) => path.includes("creche-lifecycle") || path.includes("creche-physical") || path.includes("creche-graduation"))).toBe(false);
  await page.reload();
  await expect(page.getByRole("heading", { name: "Bodies begin somewhere" })).toBeVisible();
  await expect(page.locator(".body-birth-runner")).toHaveCount(0);
});

test("the standalone Crèche runs the same durable birth and graduation path without Book assets", async ({ page }) => {
  entrance.child.kill();
  entrance = await startCreche();
  const responses = [];
  page.on("response", (response) => responses.push(new URL(response.url()).pathname));
  await page.goto(entrance.url);
  await expect(page).toHaveTitle("Conduit Crèche");
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const birth = page.locator(".body-birth-runner");
  await birth.getByLabel("Friendly Body name").fill("standalone firefly");
  await birth.getByRole("button", { name: "Birth Body" }).click();
  const bodyId = await birth.getAttribute("data-body-id");
  expect(bodyId).toMatch(/^[0-9a-f]{64}$/);
  await page.getByRole("button", { name: "2. First Host" }).click();
  await page.getByRole("button", { name: "Give this Body its first Host" }).click();
  await page.getByRole("button", { name: "4. Graduate" }).click();
  await page.getByRole("button", { name: "Finish without hosted Patchbay" }).click();
  await expect(page.locator(".graduation-runner")).toHaveAttribute("data-body-id", bodyId);
  await expect(page.locator(".body-biography li")).toHaveCount(4);
  const durable = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    api.conduit_creche_biography();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return JSON.parse(new TextDecoder().decode(bytes));
  });
  expect(durable.body_id).toBe(bodyId);
  expect(durable.schema).toBe("conduit.body/biography-evidence@1");
  expect(responses.some((path) => path.startsWith("/book/") || path.includes("chapter-"))).toBe(false);
});

test("the standalone Crèche birth controls remain separated at a narrow viewport", async ({ page }) => {
  entrance.child.kill();
  entrance = await startCreche();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const runner = page.locator(".body-birth-runner");
  const [program, name, source, editor] = await Promise.all([
    runner.getByLabel("Initial program").boundingBox(),
    runner.getByLabel("Friendly Body name").boundingBox(),
    runner.locator(".seed-source").boundingBox(),
    runner.locator(".birth-editor").boundingBox(),
  ]);
  for (const box of [program, name, source, editor]) expect(box).not.toBeNull();
  expect(program.y + program.height).toBeLessThanOrEqual(name.y);
  expect(name.y + name.height).toBeLessThanOrEqual(source.y);
  for (const control of [program, name, source]) {
    expect(control.x).toBeGreaterThanOrEqual(editor.x);
    expect(control.x + control.width).toBeLessThanOrEqual(editor.x + editor.width);
  }
  await runner.locator(".seed-source summary").click();
  await expect(runner.locator(".seed-source textarea")).toBeVisible();
  const selectAppearance = await runner.getByLabel("Initial program").evaluate(
    (element) => getComputedStyle(element).appearance,
  );
  expect(selectAppearance).toBe("none");
});

test("two Bodies seal distinct spores against the same verified packaged Pico IMAGE", async ({ page }) => {
  const prepareOne = async (variant) => {
    const birth = await birthStandaloneBody(page, { sourceVariant: variant });
    await page.getByRole("button", { name: "3. Physical Host" }).click();
    const runner = page.locator(".physical-host-runner");
    await expect(runner.locator("input[type=file]")).toHaveCount(0);
    await expect(runner.locator('[data-stage="image"]')).toHaveClass(/complete/);
    await runner.getByRole("button", { name: "Prepare Body spore" }).click();
    await expect(runner.locator('[data-stage="spore"]')).toHaveClass(/complete/);
    return { birth, evidence: JSON.parse(await runner.locator("details code").textContent()) };
  };
  const first = await prepareOne("A");
  const second = await prepareOne("B");
  expect(first.birth.bodyId).not.toBe(second.birth.bodyId);
  expect(first.evidence.image.content_digest).toBe("sha256:b373071c9bf76282457a5f03e59e5d5caaba21e376076b759724434efcf2bc9d");
  expect(first.evidence.image.content_digest).toBe(second.evidence.image.content_digest);
  expect(first.evidence.image.artifact_id).toBe("conduit-pico-w-signal/pico-local-b7@1");
  expect(first.evidence.prepared.spore_id).not.toBe(second.evidence.prepared.spore_id);
  expect(first.evidence.prepared.invitation_id).not.toBe(second.evidence.prepared.invitation_id);
  expect(first.evidence.prepared.image_content_digest).toBe(second.evidence.prepared.image_content_digest);
});

test("all guided pages lead with human motivation and return to the Conduit payoff", async ({
  page,
}) => {
  const anchors = [
    "build one computer out of the computers you actually have",
    "A successful deployment is only deployment",
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
    "durable computer Conduit maintains",
    "described enduring meaning",
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

test("the substitution, explicit fan-out, and generic-verb pages execute in order", async ({ page }) => {
  await openStep(page, 2);
  const hostId = await page.evaluate(() => globalThis.__conduitBookHost.hostId);
  let runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("HELLO");
  await expect(runner.locator(".play-status")).toContainText("Completed");

  await page.getByRole("button", { name: "Next" }).click();
  await expect(page.getByRole("heading", { name: "Fan out explicitly" })).toBeVisible();
  runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("true");
  await expect(runner.locator(".play-status")).toContainText("Completed");

  await page.getByRole("button", { name: "Next" }).click();
  await expect(page.getByRole("heading", { name: "Use a generic verb" })).toBeVisible();
  runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("3.000000");
  await expect(runner.locator(".play-status")).toContainText("Completed");
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toBe(hostId);
});

test("the Back pages compare two realizations deliberately", async ({ page }) => {
  await openStep(page, 5);
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

test("Book navigation preserves executable drafts without owning lifecycle controls", async ({ page }) => {
  await openStep(page, 2);
  const hostId = await page.evaluate(() => globalThis.__conduitBookHost.hostId);
  const edited = (await page.locator("textarea").inputValue()).replace('"hello"', '"reader"');
  await page.locator("textarea").fill(edited);
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Previous" }).click();
  await expect(page.locator("textarea")).toHaveValue(edited);
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toBe(hostId);
  await expect(page.getByRole("button", { name: "Reset this page" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Revisit birth page" })).toHaveCount(0);
});

test("unsupported capability and type mismatch remain ordinary pre-Play refusals", async ({ page }) => {
  await openStep(page, 2);
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

test("state over time presents startup and current count through four admitted browser ticks", async ({ page }) => {
  await openStep(page, 8);
  await expect(page.getByRole("heading", { name: "State over time" })).toBeVisible();
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

test("stopping state over time cancels the pending timer without a late completion", async ({ page }) => {
  await openStep(page, 8);
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

test("Meet the Host shows the exact installed offers from the planning advertisement", async ({ page }) => {
  await openStep(page, 9);
  await expect(page.getByRole("heading", { name: "Meet the Host" })).toBeVisible();
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

test("Two browser Hosts executes one unchanged Form across independent Hosts", async ({ page }) => {
  await openStep(page, 10);
  await expect(page.getByRole("heading", { name: "Two browser Hosts" })).toBeVisible();
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

test("Plans and Plays compact and raw views project the same exact immutable Plan", async ({ page }) => {
  await openStep(page, 11);
  await expect(page.getByRole("heading", { name: "Plans and Plays" })).toBeVisible();
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

test("Add a physical Host keeps IMAGE, deployment, Boot, join, admission, offers, Plan, and Play distinct", async ({ page }) => {
  await installB7Devices(page);
  const birth = await birthStandaloneBody(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
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

test("Add a physical Host retains a refused WebUSB acquisition as terminal", async ({ page }) => {
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
  await birthStandaloneBody(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await expect(runner.locator('[data-stage="image"]')).toHaveClass(/complete/);
  await runner.getByRole("button", { name: "Prepare Body spore" }).click();
  const deploy = runner.getByRole("button", { name: "Connect BOOTSEL and deploy" });
  await deploy.click();
  await expect(runner.locator(".physical-status")).toContainText("This USB acquisition is terminal");
  await expect(runner.locator('[data-stage="deploy"] span')).toHaveText("waiting");
  await expect(deploy).toBeDisabled();
});

test("Add a physical Host retains the exact Picoboot refusal chain", async ({ page }) => {
  await installB7Devices(page, { staleStatus: true });
  await birthStandaloneBody(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await expect(runner.locator('[data-stage="image"]')).toHaveClass(/complete/);
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

test("graduation retains the same Body through an ordinary hosted Patchbay Plan", async ({ page }) => {
  const { bodyId } = await birthStandaloneBody(page, { attachFirstHost: true });
  await page.getByRole("button", { name: "4. Graduate" }).click();
  const runner = page.locator(".graduation-runner");
  await expect(runner.locator(".graduation-criteria li.ready")).toHaveCount(3);
  await runner.getByRole("button", { name: "Host Patchbay on this Body" }).click();
  await expect(runner).toHaveAttribute("data-body-id", bodyId);
  await expect(runner.locator(".graduation-evidence")).toContainText("browser/patchbay-surface@1");
  await expect(runner.locator(".graduation-evidence")).toContainText("Crèche requiredfalse");
  const biography = runner.locator(".body-biography");
  await expect(biography).toHaveAttribute("data-body-id", bodyId);
  await expect(biography.locator("li")).toHaveCount(4);
  await expect(biography.locator("strong")).toHaveText(["Born", "Part admitted", "Host joined", "Graduated from the Crèche"]);
  await expect(biography).toContainText("browser/patchbay-surface@1");
  await runner.getByRole("button", { name: "End the Crèche" }).click();
  await expect(page.locator(".creche-complete")).toContainText(bodyId);
  await expect(page.locator(".creche-steps")).toHaveCount(0);
  await expect(page.locator(".creche-complete .body-biography li")).toHaveCount(4);
  const retained = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    api.conduit_creche_current();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return JSON.parse(new TextDecoder().decode(bytes));
  });
  expect(retained.body_id).toBe(bodyId);
  expect(retained.graduation.choice).toBe("host-patchbay");
  expect(retained.graduation.patchbay_plan_id).toMatch(/^[0-9a-f]{64}$/);
  const durable = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    api.conduit_creche_biography();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return JSON.parse(new TextDecoder().decode(bytes));
  });
  expect(durable.body_id).toBe(bodyId);
  expect(durable.records).toHaveLength(4);
});

test("graduation can finish without hosting Patchbay and still retain the same Body", async ({ page }) => {
  const { bodyId } = await birthStandaloneBody(page, { attachFirstHost: true });
  await page.getByRole("button", { name: "4. Graduate" }).click();
  const runner = page.locator(".graduation-runner");
  await runner.getByRole("button", { name: "Finish without hosted Patchbay" }).click();
  await expect(runner).toHaveAttribute("data-body-id", bodyId);
  await expect(runner.locator(".graduation-evidence")).toContainText("Patchbay Plannot hosted");
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.choice).toBe("external-reader");
  expect(evidence.patchbay_plan_id).toBeNull();
  expect(evidence.creche_required).toBe(false);
  await expect(runner.locator(".body-biography li")).toHaveCount(4);
  await expect(runner.locator(".body-biography")).toContainText("compatible reader can project this same evidence later");
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
  await openStep(page, 10);
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
