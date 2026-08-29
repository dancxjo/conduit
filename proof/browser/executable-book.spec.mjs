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

test.beforeEach(async () => {
  entrance = await startBook();
});

test.afterEach(() => entrance?.child.kill());

test("an edited inline Form plans and manifests through the browser Host", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  const runner = page.locator(".runner").first();
  const listing = runner.locator("#listing");
  expect(await listing.inputValue()).toContain('message: text/literal("SOS")');
  await listing.fill(
    (await listing.inputValue()).replace('"SOS"', '"E"').replace("(120)", "(40)"),
  );
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("·");
  await expect(runner.locator(".play-status")).toContainText("Completed");
  await expect(runner.locator("details dd")).toHaveCount(12);
  await expect(runner.locator("details")).not.toHaveAttribute("open", "");
  const identities = await runner.locator("details dd").allTextContents();
  expect(identities.every((identity) => identity.length > 8)).toBe(true);
  await expect(runner.locator("details")).not.toContainText("do not retain");
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toMatch(/^browser\//);
});

test("an out-of-scope Form is refused before Play", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  const runner = page.locator(".runner").first();
  const listing = runner.locator("#listing");
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
  await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
});

test("the truthful palette and eight inline runners come from one browser Host", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.locator(".runner")).toHaveCount(8);
  await expect(page.getByLabel("Morse realization")).toHaveCount(0);
  await expect(page.locator(".gear-inventory summary")).toContainText("exact browser implementations");
  await expect(page.locator(".gear-inventory summary")).toContainText("16 Gear / 24 Cord bound");
  await expect(page.locator(".gear-inventory li.available")).toHaveCount(25);
  await expect(page.locator(".gear-inventory li.unavailable")).toHaveCount(5);
  await expect(page.locator(".gear-inventory")).toContainText("time/delay");
  await expect(page.locator(".gear-inventory")).toContainText("browser-resource-or-authority-pending");
});

test("typed text makes an exact Morse round trip through the same browser Host", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  const runner = page.locator(".runner").nth(7);
  const listing = runner.locator("textarea");
  await listing.fill((await listing.inputValue()).replace('"HELLO 2"', '"SOS 2"'));
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("SOS 2");
  await expect(runner.locator(".play-status")).toContainText("Completed");
  await expect(runner.locator("details")).toContainText("Source interaction proposal");
  await expect(runner.locator("details")).toContainText("Source interaction result");
  await expect(runner.locator("details")).not.toContainText("SOS 2");
  await expect(runner.locator(".expansion")).toContainText("text/morse");
  await expect(runner.locator(".expansion")).toContainText("morse/text");
});

test("one deliberate comparison reveals direct and recursive substitution", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  const comparison = page.locator(".realization-comparison");
  const direct = comparison.locator(".runner").nth(0);
  const recursive = comparison.locator(".runner").nth(1);
  const listing = direct.locator("textarea");
  await listing.fill((await listing.inputValue()).replace('"HELLO"', '"E"').replace("(80)", "(40)"));
  await expect(recursive.locator("textarea")).toHaveValue(await listing.inputValue());
  await direct.getByRole("button", { name: "Run Host leaf" }).click();
  await expect(direct.locator(".morse")).toHaveText("·");
  await expect(direct.locator(".play-status")).toContainText("Completed");
  await recursive.getByRole("button", { name: "Run open Back" }).click();
  await expect(recursive.locator(".morse")).toHaveText("·");
  await expect(recursive.locator(".play-status")).toContainText("Completed");
  await expect(direct.locator(".expansion")).toContainText("Selected realization: direct");
  await expect(recursive.locator(".expansion")).toContainText("Selected realization: recursive");
  await expect(recursive.locator(".expansion")).toContainText("text/morse-symbols");
  await expect(recursive.locator(".expansion")).toContainText("morse/lookup");
  await expect(recursive.locator(".expansion")).toContainText("morse/symbols-to-pattern");
  const directIdentities = await direct.locator("details dd").allTextContents();
  const recursiveIdentities = await recursive.locator("details dd").allTextContents();
  expect(directIdentities[0]).toBe(recursiveIdentities[0]);
  expect(directIdentities[1]).toBe(recursiveIdentities[1]);
  expect(directIdentities[2]).not.toBe(recursiveIdentities[2]);
  expect(directIdentities[3]).not.toBe(recursiveIdentities[3]);
});

test("math logic fanout and structured language edit and run without reloading", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  const hostId = await page.evaluate(() => globalThis.__conduitBookHost.hostId);
  const cases = [
    { index: 3, result: "3.000000" },
    { index: 4, result: "false" },
    { index: 5, result: "true" },
    { index: 6, result: /4 linguistic annotations/ },
  ];
  for (const specimen of cases) {
    const runner = page.locator(".runner").nth(specimen.index);
    await runner.getByRole("button", { name: "Run" }).click();
    await expect(runner.locator(".morse")).toHaveText(specimen.result);
    await expect(runner.locator(".play-status")).toContainText("Completed");
  }
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toBe(hostId);
});
