import { expect, test } from "@playwright/test";

for (const [name, path] of [
  ["pure-node execution", "/browser/pure-node-proof.test.html"],
  ["authoritative Patchbay session", "/browser/patchbay-session-proof.test.html"],
]) {
  test(`executes the ${name} proof against the assembled Tour artifact`, async ({ page }) => {
    const failures = [];
    page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
    page.on("console", (message) => {
      if (message.type() === "error") failures.push(message.text());
    });

    await page.goto(path);
    await expect(page.locator("#result")).toHaveText("ok", { timeout: 20_000 });
    expect(failures).toEqual([]);
  });
}
