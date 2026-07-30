import { expect, test } from "@playwright/test";

test("runs a production lesson in the resolved browser worker", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });

  await page.goto("/tour/public/index.html?autorun");
  await expect(page.locator("#result")).toContainText("Hello from the Tour.", {
    timeout: 20_000,
  });
  await expect(page.locator("#result")).toContainText(
    "Evidence: 2 nodes, 1 cords conducted.",
  );
  await expect(page.locator("#execution-note")).toContainText(
    "exact dedicated-worker placement",
  );
  await expect(page.locator("#plan")).toContainText(
    "conduit/hosted-literal-v1",
  );
  await expect(page.locator("#plan")).toContainText("bound-in-this-plan");
  await expect(page.locator("#evidence")).toContainText('"event_kind": "terminal"');
  await expect(page.locator("#evidence")).toContainText('"terminal_cause": "succeeded"');
  expect(failures).toEqual([]);
});

test("runs with Shift+Enter from editor and workspace focus", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  const result = page.locator("#result");

  await expect(page.locator("#run")).toHaveAttribute(
    "aria-keyshortcuts",
    "Shift+Enter",
  );
  await expect(page.locator("#run")).toBeEnabled();
  await source.focus();
  await page.keyboard.press("Shift+Enter");
  await expect(result).toContainText("Hello from the Tour.", {
    timeout: 20_000,
  });

  await source.fill(
    (await source.inputValue()).replace("Hello from the Tour.", "Workspace shortcut."),
  );
  await expect(result).toContainText("Valid runnable panel");
  await page.locator("#check").focus();
  await page.keyboard.press("Shift+Enter");
  await expect(result).toContainText("Workspace shortcut.", {
    timeout: 20_000,
  });
});

test("preserves a recoverable draft across reset", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await source.fill((await source.inputValue()).replace("Hello from the Tour.", "Recover me."));
  await page.locator("#reset").click();
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await page.locator("#undo-reset").click();
  await expect(source).toHaveValue(/Recover me\./);
});

test("highlights panel source while retaining the native editor surface", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  const highlight = page.locator(".panel-source-highlight");

  await expect(source).toHaveAttribute("data-highlighting", "panel");
  await expect(highlight.locator(".panel-token-keyword").first()).toHaveText("panel");
  await expect(highlight.locator(".panel-token-type").first()).toHaveText("std/literal");
  await expect(
    highlight.locator(".panel-token-string").filter({ hasText: "Hello from the Tour." }),
  ).toHaveCount(1);

  await source.fill(
    "panel 2\n# note\ninterface speech/recognizer {\n" +
      "  input audio : audio/pcm-stream\n" +
      "}\nnode value : fixture/source implements speech/recognizer\n",
  );
  await expect(highlight.locator(".panel-token-comment")).toHaveText("# note");
  await expect(highlight.locator(".panel-token-type")).toHaveText([
    "audio/pcm-stream",
    "fixture/source",
    "speech/recognizer",
  ]);
  await expect(highlight.locator(".panel-token-identifier").filter({
    hasText: "speech/recognizer",
  })).toHaveCount(1);
  const typeColor = await highlight.locator(".panel-token-type").first().evaluate(
    (element) => getComputedStyle(element).color,
  );
  const identifierColor = await highlight.locator(".panel-token-identifier").first().evaluate(
    (element) => getComputedStyle(element).color,
  );
  expect(typeColor).not.toBe(identifierColor);
  await expect(source).toHaveValue(
    "panel 2\n# note\ninterface speech/recognizer {\n" +
      "  input audio : audio/pcm-stream\n" +
      "}\nnode value : fixture/source implements speech/recognizer\n",
  );
  await expect(highlight).toHaveAttribute("aria-hidden", "true");
});

test("covers Chapters 0-3 and exposes production topology projections", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await expect(page.locator("#lessons > li")).toHaveCount(16);
  await page.getByRole("button", { name: "Inside / outside" }).click();
  await expect(page.locator("#source")).toHaveValue(/example\/upper-box/);
  await page.locator("#expanded-view").click();
  await expect(page.locator("#topology")).toContainText(
    "box.worker : text/uppercase",
  );
  await page.locator("#logical-view").click();
  await expect(page.locator("#topology")).toContainText(
    "composite box : example/upper-box",
  );
});

test("accepts a semantically correct alternate solution", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await source.fill(
    (await source.inputValue())
      .replace("node greeting ", "node salutation ")
      .replace("greeting.out", "salutation.out"),
  );
  await page.locator("#run").click();
  await expect(page.locator("#result")).toContainText("✓ Lesson complete!", {
    timeout: 20_000,
  });
  await expect(source).toHaveValue(/node salutation/);
});

test("uses React Flow with legacy line placement disabled", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const canvas = page.locator("#patchbay-flow-root");
  await expect(canvas).toHaveAttribute("data-renderer", "react-flow");
  await expect(canvas).toHaveAttribute("data-projection", "rust-authoritative-v1");
  await expect(canvas).toHaveAttribute("data-legacy-line-placement", "false");
  await expect(canvas).toHaveAttribute("data-node-count", "2");
  await expect(canvas).toHaveAttribute("data-edge-count", "1");
  await expect(page.locator(".conduit-faceplate-card")).toHaveCount(2, {
    timeout: 20_000,
  });
  const canvasBox = await canvas.boundingBox();
  const firstNodeBox = await page.locator(".react-flow__node").first().boundingBox();
  expect(canvasBox?.height).toBeGreaterThan(0);
  expect(firstNodeBox?.y).toBeGreaterThanOrEqual(canvasBox?.y ?? Infinity);
  expect(firstNodeBox?.y).toBeLessThan((canvasBox?.y ?? 0) + (canvasBox?.height ?? 0));
  await expect(page.locator(".availability-tag")).toHaveCount(2);
});

test("shows node movement while a topology box is being dragged", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const node = page.locator(".react-flow__node").first();
  await node.scrollIntoViewIfNeeded();
  const before = await node.boundingBox();
  expect(before).not.toBeNull();
  const beforeTransform = await node.evaluate((element) => element.style.transform);

  const startX = before.x + before.width / 2;
  const startY = before.y + 20;
  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(startX + 80, startY + 32, { steps: 4 });

  const during = await node.boundingBox();
  expect(during.x).toBeGreaterThan(before.x + 40);
  expect(during.y).toBeGreaterThan(before.y + 15);

  await page.mouse.up();
  await expect.poll(
    async () => node.evaluate((element) => element.style.transform),
  ).not.toBe(
    beforeTransform,
  );
});

test("retains committed topology positions across renders and visits", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const greeting = page.locator('[data-id="greeting"]');
  await greeting.scrollIntoViewIfNeeded();
  const before = await greeting.boundingBox();
  expect(before).not.toBeNull();
  const beforeTransform = await greeting.evaluate((element) => element.style.transform);

  const startX = before.x + before.width / 2;
  const startY = before.y + 20;
  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(startX + 96, startY + 48, { steps: 4 });
  await page.mouse.up();
  await expect.poll(
    async () => greeting.evaluate((element) => element.style.transform),
  ).not.toBe(beforeTransform);
  const committedTransform = await greeting.evaluate(
    (element) => element.style.transform,
  );

  await page.locator("#check").click();
  await expect(greeting).toHaveCSS("transform", /matrix/);
  await expect.poll(
    async () => greeting.evaluate((element) => element.style.transform),
  ).toBe(committedTransform);

  await page.getByRole("button", { name: "Inside / outside" }).click();
  await page.getByRole("button", { name: "Hello, panel" }).click();
  await expect.poll(
    async () => greeting.evaluate((element) => element.style.transform),
  ).toBe(committedTransform);

  await page.reload();
  await expect.poll(
    async () => greeting.evaluate((element) => element.style.transform),
  ).toBe(committedTransform);
});

test("retains headless editing and execution when presentation fails", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.evaluate(() => {
    window.__CONDUIT_DISABLE_PATCHBAY_RENDERER__ = true;
  });
  const source = page.locator("#source");
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await source.evaluate((element) => {
    element.value = element.value.replace("Hello from the Tour.", "Headless proof.");
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await expect(page.locator("#result")).toContainText("Valid runnable panel");
  await expect(page.locator("#cy")).toContainText("React Flow renderer unavailable.");
  await page.locator("#run").click();
  await expect(page.locator("#result")).toContainText("Headless proof.", {
    timeout: 20_000,
  });
});

test("styles cords from their projected type and pressure policy", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const edge = page.locator(".patchbay-smart-cord").first();
  await expect(edge).toHaveClass(/pressure-block/);
  await expect(edge).toHaveClass(/pressure-lossless/);
  await expect(edge).toHaveClass(/value-type-std-text/);
  await expect(edge).toHaveClass(/type-family-text/);
  await expect(edge).toHaveClass(/capacity-single/);
  await expect(edge).toHaveClass(/compatibility-compatible/);
  const path = edge.locator(".react-flow__edge-path");
  await expect(path).toHaveAttribute("d", /^M/);
  await expect(path).toHaveCSS("stroke", "rgb(52, 211, 153)");
  await expect(path).toHaveCSS("animation-name", "patchbay-cord-block");
  await expect(edge.locator(".react-flow__edge-text")).toContainText(
    "1 slots · 0↗1 · block(fifo)",
  );
  await expect(page.locator(".cord-legend-item")).toHaveCount(4);
});

test("reference panels expose canonical contract-only status and disable Run", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.getByRole("button", { name: "File Copier Pipeline" }).click();
  await expect(page.locator("#runnability-state")).toContainText("contract-only");
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#result")).toContainText("CND-IMP-001");
  await expect(page.locator("#source")).toHaveValue(/node reader : std\/file-read/);
});

test("pedagogical completion is not execution evidence", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.getByRole("button", { name: "Pull the cord" }).click();
  await expect(page.locator("#run")).toBeDisabled();
  await page.locator("#check").click();
  await expect(page.locator("#result")).toContainText(
    "Lesson check complete (not execution evidence)",
  );
  await expect(page.locator("#evidence")).toContainText(
    '"executionEvidence": false',
  );
});

test("illustrative lessons cannot run their pedagogical target", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.getByRole("button", { name: "More than one port" }).click();
  await expect(page.locator("#runnability-state")).toContainText(
    "illustrative/unavailable",
  );
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#result")).toContainText("CND-CMP-006");
  await expect(page.locator("#evidence")).not.toContainText('"event_kind": "terminal"');
});

test("typed formatter lesson shares exact graph and ordered evidence scenarios", async ({ page }) => {
  await page.goto(
    "/tour/public/index.html?lesson=library.typed-text-format",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");
  const source = page.locator("#source");

  await expect(story).toBeVisible();
  await expect(story).toContainText("std/text/format");
  await expect(story).toContainText("std/format-values/literal");
  await story.getByRole("button", { name: "std/text/format" }).click();
  await expect(page.locator("#selected-node-label")).toContainText("message");
  await expect(story.locator("#library-docs a")).toHaveCount(4);
  await expect(page.locator("#scenario option")).toHaveCount(4);
  await expect(page.locator('[data-id="message"]')).toContainText("std/text/format");
  await expect(page.locator('[data-id="message"]')).toContainText("template");
  await expect(page.locator('[data-id="message"]')).toContainText("values");

  await page.locator("#run").click();
  await expect(result).toContainText("Hello, operator.", { timeout: 20_000 });
  await expect(page.locator("#timeline-table tbody tr")).not.toHaveCount(0);
  await expect(page.locator("#timeline-table")).toContainText("succeeded");
  await expect(page.locator("#timeline-table")).toContainText("block");
  await expect(page.locator("#timeline-values")).toContainText(
    'Exact stdout: "Hello, operator.\\n"',
  );
  await expect(page.locator("#timeline-position-label")).toContainText(/of \d+: terminal/);

  await page.locator("#timeline-reset").click();
  await expect(page.locator("#timeline-position-label")).toContainText("1 of");
  await page.locator("#timeline-step").click();
  await expect(page.locator("#timeline-position-label")).toContainText("2 of");
  await story.focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.locator("#timeline-position-label")).toContainText("3 of");

  await page.locator("#scenario").selectOption("composition");
  await page.locator("#run").click();
  await expect(result).toContainText("HELLO, OPERATOR.", { timeout: 20_000 });
  await expect(page.locator('[data-id="shout"]')).toContainText("text/uppercase");

  await page.locator("#scenario").selectOption("missing-value");
  await page.locator("#run").click();
  await expect(result).toContainText("format/missing-value", { timeout: 20_000 });
  await expect(page.locator("#timeline-table")).toContainText(/failed|rejected/);
  await expect(page.locator("#timeline-values")).toContainText(
    "Exact run rejection: format/missing-value",
  );

  await page.locator("#scenario").selectOption("cancelled");
  await page.locator("#run").click();
  await expect(result).toContainText("cancelled", { timeout: 20_000 });
  await expect(page.locator("#timeline-table")).toContainText("cancelled");

  await page.locator("#scenario").selectOption("standalone");
  await source.fill((await source.inputValue()).replace("operator", "robot"));
  await page.locator("#run").click();
  await expect(result).toContainText("Hello, robot.", { timeout: 20_000 });
});
