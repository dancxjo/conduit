import { expect, test } from "@playwright/test";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

function startServer() {
  const child = spawn("target/debug/patchbay-html", ["--debugger-watch-fixture"], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  const errors = [];
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", chunk => errors.push(chunk));
  const lines = createInterface({ input: child.stdout });
  const url = new Promise((resolve, reject) => {
    lines.once("line", line => resolve(line.replace("PATCHBAY_HTML_URL=", "")));
    child.once("exit", code => reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));
  });
  return { child, lines, url };
}

test("an exact Cord Watch is keyboard operable, finite, and survives reload", async ({ page }) => {
  const server = startServer();
  try {
    const url = await server.url;
    const initial = await (await fetch(`${url}/api/snapshot`)).json();
    const cord = initial.watches.eligible_subjects.find(([, role]) => role === "cord")[0];

    await page.goto(url);
    await page.locator("#toggle-palette").click();
    const subject = page.locator(`#subjects button[data-subject="${cord}"]`);
    await subject.focus();
    await subject.press("Enter");

    const addWatch = page.getByRole("button", { name: "Watch", exact: true });
    await addWatch.focus();
    await addWatch.press("Enter");
    const card = page.locator(".watch-card").filter({ has: page.getByRole("button", { name: `Watch ${cord}`, exact: true }) });
    await expect(card).toContainText("42");
    await expect(card).toContainText("scalar");
    await expect(card).toContainText("Latest sequence42");
    await expect(card).toContainText("2 observations lost before sequence 40");
    await expect(page.getByRole("list", { name: `Recent observations for ${cord}` }).getByRole("listitem")).toHaveCount(1);

    const afterAdd = await (await fetch(`${url}/api/snapshot`)).json();
    expect(afterAdd.presentation).toEqual(initial.presentation);
    expect(afterAdd.watches.watches).toHaveLength(1);
    await page.reload();
    const afterReload = await (await fetch(`${url}/api/snapshot`)).json();
    expect(afterReload.watches.watches.map(item => item.subject)).toEqual([cord]);
    await page.locator("#toggle-palette").click();
    await page.locator(`#subjects button[data-subject="${cord}"]`).click();
    await expect(page.getByRole("button", { name: `Watch ${cord}`, exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Clear Watch history", exact: true }).click();
    await expect(page.getByRole("list", { name: `Recent observations for ${cord}` }).getByRole("listitem")).toHaveCount(0);
    await page.getByRole("button", { name: "Remove Watch", exact: true }).click();
    await expect(page.locator(".watch-card")).toHaveCount(0);
    const afterRemove = await (await fetch(`${url}/api/snapshot`)).json();
    expect(afterRemove.presentation).toEqual(initial.presentation);
  } finally {
    server.lines.close();
    server.child.kill("SIGTERM");
  }
});
