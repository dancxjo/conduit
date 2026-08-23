import { spawn } from "node:child_process";
import { expect, test } from "@playwright/test";

const entrances = [];

async function startEntrance() {
  const child = spawn("target/debug/conduit-browser-host", ["--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  entrances.push(child);
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error(`browser Host entrance was not ready\n${output}`)),
      10_000,
    );
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/CONDUIT_BROWSER_HOST_URL=(http:\/\/127\.0\.0\.1:\d+\/)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`browser Host entrance exited (${code})\n${output}`));
    });
  });
  return { child, url };
}

test.afterEach(() => {
  while (entrances.length > 0) entrances.pop().kill();
});

test("independent entrances and reload own fresh page WASM Host truth", async ({ browser }) => {
  const firstEntrance = await startEntrance();
  const secondEntrance = await startEntrance();
  expect(firstEntrance.url).not.toBe(secondEntrance.url);

  const firstPage = await browser.newPage();
  const secondPage = await browser.newPage();
  await Promise.all([firstPage.goto(firstEntrance.url), secondPage.goto(secondEntrance.url)]);
  await Promise.all([
    expect(firstPage.getByRole("status")).toHaveText("Current and independently initialized"),
    expect(secondPage.getByRole("status")).toHaveText("Current and independently initialized"),
  ]);
  const identity = (page) => page.evaluate(() => ({
    hostId: globalThis.__conduitBrowserHost.hostId,
    bootId: globalThis.__conduitBrowserHost.bootId,
  }));
  const firstIdentity = await identity(firstPage);
  const secondIdentity = await identity(secondPage);
  expect(firstIdentity.hostId).not.toBe(secondIdentity.hostId);
  expect(firstIdentity.bootId).not.toBe(secondIdentity.bootId);
  await expect(firstPage.getByText("None", { exact: true })).toBeVisible();
  await expect(secondPage.getByText("None", { exact: true })).toBeVisible();

  await firstPage.reload();
  await expect(firstPage.getByRole("status")).toHaveText("Current and independently initialized");
  const replacementIdentity = await identity(firstPage);
  expect(replacementIdentity.hostId).not.toBe(firstIdentity.hostId);
  expect(replacementIdentity.bootId).not.toBe(firstIdentity.bootId);
  expect(await identity(secondPage)).toEqual(secondIdentity);
});
