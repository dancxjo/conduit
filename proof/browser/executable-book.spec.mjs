import { spawn } from "node:child_process";
import { expect, test } from "@playwright/test";

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
  await expect(page.locator(".tour-progress")).toHaveText(`Step ${index + 1} of 9`);
}

test.beforeEach(async () => {
  entrance = await startBook();
});

test.afterEach(() => entrance?.child.kill());

test("Step 0 edits and runs one ordinary Form before introducing architecture", async ({ page }) => {
  await openStep(page, 0);
  await expect(page.getByRole("heading", { name: "Step 0 — Hello, light" })).toBeVisible();
  await expect(page.locator(".runner")).toHaveCount(1);
  await expect(page.locator(".gear-inventory")).toHaveCount(0);
  const runner = page.locator(".runner");
  const listing = runner.locator("#listing");
  await listing.fill(
    (await listing.inputValue()).replace('"SOS"', '"E"').replace("(120)", "(40)"),
  );
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("·");
  await expect(runner.locator(".play-status")).toContainText("Completed");
  await expect(runner.locator("details")).not.toHaveAttribute("open", "");
  await expect(runner.locator("details dd")).toHaveCount(12);
  const identities = await runner.locator("details dd").allTextContents();
  expect(identities.every((identity) => identity.length > 8)).toBe(true);
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toMatch(/^browser\//);
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
  await expect(runner.locator(".expansion")).toContainText("Opened reusable Forms");
  await expect(runner.locator(".expansion")).toContainText("morse/lookup");

  await page.getByRole("button", { name: "Next" }).click();
  runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("SOS 2");
  await expect(runner.locator(".expansion")).toContainText("text/morse");
  await expect(runner.locator(".expansion")).toContainText("morse/text");

  await page.getByRole("button", { name: "Next" }).click();
  const comparison = page.locator(".realization-comparison");
  const direct = comparison.locator(".runner").nth(0);
  const recursive = comparison.locator(".runner").nth(1);
  const listing = direct.locator("textarea");
  await listing.fill((await listing.inputValue()).replace('"HELLO"', '"E"'));
  await expect(recursive.locator("textarea")).toHaveValue(await listing.inputValue());
  await direct.getByRole("button", { name: "Run Host leaf" }).click();
  await expect(direct.locator(".play-status")).toContainText("Completed");
  await recursive.getByRole("button", { name: "Run open Back" }).click();
  await expect(recursive.locator(".play-status")).toContainText("Completed");
  await expect(direct.locator(".morse")).toHaveText("·");
  await expect(recursive.locator(".morse")).toHaveText("·");
  const directIdentities = await direct.locator("details dd").allTextContents();
  const recursiveIdentities = await recursive.locator("details dd").allTextContents();
  expect(directIdentities[0]).toBe(recursiveIdentities[0]);
  expect(directIdentities[1]).toBe(recursiveIdentities[1]);
  expect(directIdentities[2]).not.toBe(recursiveIdentities[2]);
  expect(directIdentities[3]).not.toBe(recursiveIdentities[3]);
});

test("Tour navigation preserves drafts but reset and restart change presentation state only", async ({ page }) => {
  await openStep(page, 1);
  const hostId = await page.evaluate(() => globalThis.__conduitBookHost.hostId);
  const edited = (await page.locator("textarea").inputValue()).replace('"hello"', '"reader"');
  await page.locator("textarea").fill(edited);
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Previous" }).click();
  await expect(page.locator("textarea")).toHaveValue(edited);
  await page.getByRole("button", { name: "Reset this step" }).click();
  await expect(page.locator("textarea")).toHaveValue(/"hello"/);
  await page.getByRole("button", { name: "Restart Tour" }).click();
  await expect(page.locator(".tour-progress")).toHaveText("Step 1 of 9");
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toBe(hostId);
});

test("unsupported capability and type mismatch remain ordinary pre-Play refusals", async ({ page }) => {
  await openStep(page, 0);
  const runner = page.locator(".runner");
  const listing = runner.locator("textarea");
  await listing.fill(`form unavailable {
    source: text/literal("still planned")
    result: presentation/text
    missing: presentation/bool
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
    "time/every", "state/count", "presentation/count",
  ]));
  await expect(inventory.locator("li.unavailable", { hasText: "input/keyboard" })).toHaveCount(1);
});
