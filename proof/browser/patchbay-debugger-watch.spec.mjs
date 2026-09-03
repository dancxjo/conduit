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

function startLearnedServer() {
  const child = spawn("target/debug/patchbay-learned-watch-proof", [], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  const errors = [];
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", chunk => errors.push(chunk));
  const lines = createInterface({ input: child.stdout });
  const url = new Promise((resolve, reject) => {
    lines.once("line", line => resolve(line.replace("PATCHBAY_HTML_URL=", "")));
    child.once("exit", code => reject(new Error(`Learned Patchbay exited ${code}: ${errors.join("")}`)));
  });
  return { child, lines, url };
}

test("the real Patchbay shows one bounded Tongues system across signals, belief, state, loss, and dynamics", async ({ page }) => {
  const server = startLearnedServer();
  try {
    const url = await server.url;
    const initial = await (await fetch(`${url}/api/snapshot`)).json();
    expect(initial.watches.watches).toHaveLength(3);
    expect(initial.watches.watches.flatMap(watch => watch.learned_projections)).toHaveLength(15);
    await page.goto(url);
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    await expect(page.locator(".watch-card")).toHaveCount(3);
    const activeCord = page.locator(".react-flow__edge.debugger-active .react-flow__edge-path");
    await expect(activeCord).not.toHaveCount(0);
    expect(await activeCord.first().evaluate(element => getComputedStyle(element).animationName)).toBe("conduit-cord-flow");
    await expect(page.locator('.learned-watch[data-projection="signal"]')).toHaveCount(10);
    for (const channel of ["analysis/relative-phase", "analysis/label-free-events", "analysis/post-freeze-clusters", "post-hoc/annotation-boundaries", "sparse-dynamics/observed-delta", "sparse-dynamics/predicted-delta"]) {
      await expect(page.locator('.learned-watch[data-projection="signal"]').filter({ hasText: channel })).toHaveCount(1);
    }
    await expect(page.locator('.learned-watch[data-projection="tensor"]')).toContainText("i64 [16 × 4]");
    await expect(page.locator('.learned-watch[data-projection="tensor"]')).toContainText("truncated");
    await expect(page.locator('.learned-watch[data-projection="probabilistic"]')).toContainText("inferred");
    await expect(page.locator('.learned-watch[data-projection="probabilistic"]')).toContainText("conditional-sample-0");
    await expect(page.locator('.learned-watch[data-projection="state"]')).toContainText("Generation1 · step 48");
    await expect(page.locator('.learned-watch[data-projection="state"]')).toContainText("committed");
    await expect(page.locator('.learned-watch[data-projection="training"]')).toContainText("latent-agreement 2.402 + dynamics 0.885 = 3.288");
    await expect(page.locator('.learned-watch[data-projection="training"]')).toContainText("sha256:96a72fac6e92cd6c797699aff0e059f454d2bfff0fa927226c979e5ad8475c97");
    await expect(page.locator('.learned-watch[data-projection="dynamics"]')).toContainText("619520 work");
    await expect(page.locator('.learned-watch[data-projection="signal"]').filter({ hasText: "PB2007 synchronized audio/EMA derivation" })).toHaveCount(1);
    await expect(page.locator(".signal-gap")).toHaveCount(0);
    await expect(page.locator('.learned-plot circle[data-disposition="observed"]')).not.toHaveCount(0);
    await expect(page.locator('.learned-plot circle[data-disposition="inferred"]')).not.toHaveCount(0);
    await expect(page.locator(".watch-card").first()).toContainText("0 dropped");
    const final = await (await fetch(`${url}/api/snapshot`)).json();
    expect(final.presentation).toEqual(initial.presentation);
    expect(final.debugger).toEqual(initial.debugger);
  } finally {
    server.lines.close();
    server.child.kill("SIGTERM");
  }
});

test("an exact Cord Watch is keyboard operable, finite, and survives reload", async ({ page }) => {
  const server = startServer();
  try {
    const url = await server.url;
    const initial = await (await fetch(`${url}/api/snapshot`)).json();
    const cord = initial.watches.eligible_subjects.find(([, role]) => role === "cord")[0];

    await page.goto(url);
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    const admitted = await page.evaluate(() => ({
      application: globalThis.__conduitBrowserApplication.manifest.applicationId,
      packageDigest: globalThis.__conduitBrowserApplication.manifest.packageDigest,
      resourceRoles: globalThis.__conduitBrowserApplication.manifest.resources.map(({ role }) => role),
    }));
    expect(admitted.application).toBe("conduit.application/patchbay");
    expect(admitted.packageDigest).toMatch(/^sha256:[0-9a-f]{64}$/);
    expect(admitted.resourceRoles.length).toBeLessThanOrEqual(32);
    expect(new Set(admitted.resourceRoles).size).toBe(admitted.resourceRoles.length);
    expect(admitted.resourceRoles).toContain("browser-host-identity");
    await expect(page.locator('script[src="/assets/app.js"]')).toHaveCount(0);
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
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
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
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    await page.locator("#toggle-palette").click();
    await page.locator(`#subjects button[data-subject="${cord}"]`).click();
    await page.getByRole("button", { name: "Watch", exact: true }).click();

    await expect(page.locator(".timeline-status")).toContainText("Following live observations");
    await expect(page.locator(".timeline-status")).toContainText("cursor 42");
    await expect(page.locator(".timeline-gap")).toContainText("Exact reconstruction across this gap is unavailable");
    await page.getByRole("button", { name: "Pause visualization" }).click();
    await expect(page.locator(".timeline-status")).toContainText("execution is not paused");
    await page.getByRole("button", { name: "Previous event" }).click();
    await expect(page.locator(".timeline-status")).toContainText("cursor 41");
    const watch = page.locator(".watch-card");
    await expect(watch).toContainText("historical replay");
    await expect(watch).toContainText("Latest41");

    await page.locator(".timeline-events button").filter({ hasText: "seq 41" }).click();
    await expect(page.locator('.exact-selection [data-application-component="definition-table"]')).toContainText(port);
    await page.locator("#toggle-palette").click();
    await page.locator(`#subjects button[data-subject="${cord}"]`).click();
    await page.getByRole("button", { name: "Focus events for exact subject" }).click();
    await expect(page.locator('.timeline-events [data-application-component="artifact"]')).toHaveCount(2);
    await page.getByRole("button", { name: "Show all events" }).click();
    await expect(page.locator('.timeline-events [data-application-component="artifact"]')).toHaveCount(4);
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
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
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
    const exact = page.locator('.timeline-events [data-application-component="artifact"][data-causal-trace="exact"]');
    await expect(exact).toHaveCount(2);
    await expect(exact.nth(0)).toContainText("trace 1 · seq 39");
    await expect(exact.nth(1)).toContainText("trace 2 · seq 40");
    await expect(page.locator(".watch-card")).toContainText("Fault 17");
    await expect(page.locator(".causal-trace-exact")).toHaveCount(2);
    await exact.nth(0).getByRole("button").click();
    await expect(page.locator('.exact-selection [data-application-component="definition-table"]')).toContainText(cord);
    await page.getByRole("button", { name: "Clear causal trace" }).click();
    await expect(page.locator('.timeline-events [data-application-component="artifact"][data-causal-trace="exact"]')).toHaveCount(0);
    await page.locator(".timeline-events button").filter({ hasText: "seq 39" }).click();
    await page.getByRole("button", { name: "Trace downstream" }).click();
    await expect(page.locator('.timeline-events [data-application-component="artifact"][data-causal-trace="exact"]')).toHaveCount(4);
    const final = await (await fetch(`${url}/api/snapshot`)).json();
    expect(final.presentation).toEqual(initial.presentation);
  } finally {
    server.lines.close();
    server.child.kill("SIGTERM");
  }
});

test("Patchbay refuses code bytes that differ from its admitted package", async ({ page }) => {
  const server = startServer();
  try {
    const url = await server.url;
    await page.route("**/assets/app.js", async route => {
      const response = await route.fetch();
      await route.fulfill({ response, body: `${await response.text()}\n// changed after admission\n` });
    });
    await page.goto(url);
    await expect(page.locator("body")).toContainText("application resource application-module changed identity");
    await expect(page.locator("#patchbay-root")).toHaveCount(0);
  } finally {
    server.lines.close();
    server.child.kill("SIGTERM");
  }
});
