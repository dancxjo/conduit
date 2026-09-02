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
    await expect(page.locator("#status")).toHaveAttribute("data-application-revision", /^\d+$/);
    await expect(page.locator("#status [data-application-component='status']")).toContainText("Presentation revision");
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

test("timeline replay and exact event rows stay linked to the graph and Watch", async ({ page }) => {
  const server = startServer();
  try {
    const url = await server.url;
    const initial = await (await fetch(`${url}/api/snapshot`)).json();
    const cord = initial.watches.eligible_subjects.find(([, role]) => role === "cord")[0];
    const port = initial.watches.eligible_subjects.find(([, role]) => role === "port")[0];
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto(url);
    await page.locator("#toggle-palette").click();
    await page.locator(`#subjects button[data-subject="${cord}"]`).click();
    await page.getByRole("button", { name: "Watch", exact: true }).click();

    await expect(page.locator(".timeline-status")).toContainText("Following live observations");
    await expect(page.locator(".timeline-status")).toContainText("cursor 42");
    await expect(page.locator(".timeline-gap")).toContainText("Exact reconstruction across this gap is unavailable");
    await page.getByRole("button", { name: "Pause visualization" }).press("Enter");
    await expect(page.locator(".timeline-status")).toContainText("execution is not paused");
    await page.getByRole("button", { name: "Previous event" }).click();
    await expect(page.locator(".timeline-status")).toContainText("cursor 41");
    const watch = page.locator(".watch-card");
    await expect(watch).toContainText("historical replay");
    await expect(watch).toContainText("Latest41");

    await page.locator(".timeline-events button").filter({ hasText: "seq 41" }).click();
    await expect(page.locator(".exact-selection dl")).toContainText(port);
    await page.locator("#toggle-palette").click();
    await page.locator(`#subjects button[data-subject="${cord}"]`).click();
    await page.getByRole("button", { name: "Focus events for exact subject" }).click();
    await expect(page.getByRole("list", { name: "Exact retained debugger events" }).getByRole("listitem")).toHaveCount(2);
    await page.getByRole("button", { name: "Show all events" }).click();
    await expect(page.getByRole("list", { name: "Exact retained debugger events" }).getByRole("listitem")).toHaveCount(4);
    await page.getByRole("button", { name: "Jump live" }).press("Enter");
    await expect(page.locator(".timeline-status")).toContainText("Following live observations");
    await expect(watch).toContainText("Latest42");
    const final = await (await fetch(`${url}/api/snapshot`)).json();
    expect(final.presentation).toEqual(initial.presentation);
  } finally {
    server.lines.close();
    server.child.kill("SIGTERM");
  }
});

test("real breakpoint control and exact causal fault tracing remain distinct from replay", async ({ page }) => {
  const server = startServer();
  try {
    const url = await server.url;
    const initial = await (await fetch(`${url}/api/snapshot`)).json();
    const gear = initial.debugger_control.eligible_subjects[0];
    const cord = initial.watches.eligible_subjects.find(([, role]) => role === "cord")[0];
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto(url);
    await page.locator("#toggle-palette").click();
    await page.locator(`#subjects button[data-subject="${gear}"]`).click();
    await page.getByRole("button", { name: "Watch", exact: true }).click();
    await page.getByRole("button", { name: "Break here", exact: true }).click();
    await expect(page.locator(".control-status")).toContainText("Execution actually suspended");
    await expect(page.locator(".timeline-status")).toContainText("Following live observations");
    await page.getByRole("button", { name: "Pause visualization" }).click();
    await expect(page.locator(".timeline-status")).toContainText("execution is not paused");
    await expect(page.locator(".control-status")).toContainText("Execution actually suspended");
    await page.getByRole("button", { name: "Resume execution" }).click();
    await expect(page.locator(".control-status")).toContainText("Execution running");

    await page.locator(".timeline-events button").filter({ hasText: "seq 40" }).click();
    await page.getByRole("button", { name: "Trace upstream" }).click();
    const exact = page.locator('.timeline-events li[data-causal-trace="exact"]');
    await expect(exact).toHaveCount(2);
    await expect(exact.nth(0)).toContainText("trace 1 · seq 39");
    await expect(exact.nth(1)).toContainText("trace 2 · seq 40");
    await expect(page.locator(".watch-card")).toContainText("Fault 17");
    await expect(page.locator(".causal-trace-exact")).toHaveCount(2);
    await exact.nth(0).getByRole("button").click();
    await expect(page.locator(".exact-selection dl")).toContainText(cord);
    await page.getByRole("button", { name: "Clear causal trace" }).click();
    await expect(page.locator('.timeline-events li[data-causal-trace="exact"]')).toHaveCount(0);
    await page.locator(".timeline-events button").filter({ hasText: "seq 39" }).click();
    await page.getByRole("button", { name: "Trace downstream" }).click();
    await expect(page.locator('.timeline-events li[data-causal-trace="exact"]')).toHaveCount(4);
    const final = await (await fetch(`${url}/api/snapshot`)).json();
    expect(final.presentation).toEqual(initial.presentation);
  } finally {
    server.lines.close();
    server.child.kill("SIGTERM");
  }
});
