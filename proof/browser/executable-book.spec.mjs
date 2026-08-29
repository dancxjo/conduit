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
  const listing = page.locator("#listing");
  expect(await listing.inputValue()).toContain('message: text/literal("SOS")');
  await listing.fill(
    (await listing.inputValue()).replace('"SOS"', '"E"').replace("(120)", "(40)"),
  );
  await page.getByRole("button", { name: "Run" }).click();
  await expect(page.locator(".morse")).toHaveText("·");
  await expect(page.locator(".play-status")).toContainText("Completed");
  await expect(page.locator("details dd")).toHaveCount(10);
  const identities = await page.locator("details dd").allTextContents();
  expect(identities.every((identity) => identity.length > 8)).toBe(true);
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toMatch(/^browser\//);
});

test("an out-of-scope Form is refused before Play", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  const listing = page.locator("#listing");
  await listing.fill((await listing.inputValue()).replace("presentation/indicator", "text/upper"));
  await page.getByRole("button", { name: "Run" }).click();
  await expect(page.locator(".play-status")).toContainText("refused before Play");
  await expect(page.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
});
