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
  const editor = page.locator(".panel-source-editor");
  const source = page.locator("#source");
  const highlight = page.locator(".panel-source-highlight");
  const expectLayersAligned = async () => {
    await expect.poll(async () => editor.evaluate((element) => {
      const sourceBox = element.querySelector("textarea")?.getBoundingClientRect();
      const highlightBox =
        element.querySelector(".panel-source-highlight")?.getBoundingClientRect();
      if (!sourceBox || !highlightBox) return Number.POSITIVE_INFINITY;
      return Math.max(
        Math.abs(sourceBox.x - highlightBox.x),
        Math.abs(sourceBox.y - highlightBox.y),
        Math.abs(sourceBox.width - highlightBox.width),
        Math.abs(sourceBox.height - highlightBox.height),
      );
    })).toBeLessThan(0.5);
  };

  await expect(source).toHaveAttribute("data-highlighting", "panel");
  await expectLayersAligned();
  await editor.evaluate((element) => {
    element.style.height = "517px";
    element.style.width = "73%";
  });
  await expectLayersAligned();
  await expect(highlight.locator(".panel-token-keyword").first()).toHaveText("panel");
  await expect(highlight.locator(".panel-token-type").first()).toHaveText("std/literal");
  await expect(
    highlight.locator(".panel-token-string").filter({ hasText: "Hello from the Tour." }),
  ).toHaveCount(1);
  await expect(highlight.locator(".panel-token-keyword").filter({
    hasText: /^output$/,
  })).toHaveCount(0);
  await expect(highlight.locator(".panel-token-identifier").filter({
    hasText: /^output$/,
  })).toHaveCount(2);
  await expect(highlight.locator(".panel-token-port-outgoing")).toHaveText("out");
  await expect(highlight.locator(".panel-token-port-receiving")).toHaveText("in");
  await expect(highlight.locator(".panel-token-port-outgoing"))
    .toHaveAttribute("data-token-label", "outgoing port");
  await expect(highlight.locator(".panel-token-port-receiving"))
    .toHaveAttribute("data-token-label", "receiving port");
  const inputPortDecoration = await highlight.locator(".panel-token-port-receiving").evaluate(
    (element) => getComputedStyle(element).textDecorationStyle,
  );
  const outputPortDecoration = await highlight.locator(".panel-token-port-outgoing").evaluate(
    (element) => getComputedStyle(element).textDecorationStyle,
  );
  expect(inputPortDecoration).not.toBe(outputPortDecoration);

  await source.fill(
    "panel 3\n# note > ignored\ninterface speech/recognizer {\n" +
      "  > in : audio/pcm-stream\n" +
      "  in > : speech/transcript\n" +
      "  > audio : audio/pcm-stream\n" +
      "  committed > : speech/transcript\n" +
      "}\nnode value : fixture/source implements speech/recognizer\n",
  );
  await expect(highlight).toHaveAttribute("data-semantic-metadata", "available");
  await expect(highlight.locator(".panel-token-comment")).toHaveText("# note > ignored");
  await expect(highlight.locator(".panel-token-type")).toHaveText([
    "audio/pcm-stream",
    "speech/transcript",
    "audio/pcm-stream",
    "speech/transcript",
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
  await expect(highlight.locator(".panel-token-port-receiving")).toHaveText([
    "in",
    "audio",
  ]);
  await expect(highlight.locator(".panel-token-port-outgoing")).toHaveText([
    "in",
    "committed",
  ]);
  await expect(highlight.locator(".panel-token-port-sigil-receiving")).toHaveText([
    ">",
    ">",
  ]);
  await expect(highlight.locator(".panel-token-port-sigil-outgoing")).toHaveText([
    ">",
    ">",
  ]);
  await expect(highlight.locator(".panel-token-comment .panel-token-port-sigil")).toHaveCount(0);
  await expect(source).toHaveValue(
    "panel 3\n# note > ignored\ninterface speech/recognizer {\n" +
      "  > in : audio/pcm-stream\n" +
      "  in > : speech/transcript\n" +
      "  > audio : audio/pcm-stream\n" +
      "  committed > : speech/transcript\n" +
      "}\nnode value : fixture/source implements speech/recognizer\n",
  );

  await source.fill('panel 3\ninterface broken {\n  > audio : "not > metadata"\n');
  await expect(highlight).toHaveAttribute("data-semantic-metadata", "unavailable");
  await expect(highlight.locator(".panel-token-port")).toHaveCount(0);
  await expect(highlight.locator(".panel-token-port-sigil")).toHaveCount(0);
  await expect(source).toHaveValue(
    'panel 3\ninterface broken {\n  > audio : "not > metadata"\n',
  );
  await expect(highlight).toHaveAttribute("aria-hidden", "true");
});

test("covers Chapters 0-3 and exposes production topology projections", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await expect(page.locator("#lessons > li")).toHaveCount(20);
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
  const receiving = page.locator(".react-flow__node").getByRole("button", {
    name: "in, receiving port; type std/text",
    exact: true,
  });
  const outgoing = page.locator(".react-flow__node").getByRole("button", {
    name: "out, outgoing port; type std/text",
    exact: true,
  });
  await expect(receiving).toContainText("> in");
  await expect(outgoing).toContainText("out >");
  await expect(page.locator(".faceplate-jack")).not.toContainText("<");
  await expect(page.locator("#panel-port-list")).toContainText("> in");
  await expect(page.locator("#panel-port-list")).toContainText("out >");
  await outgoing.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#selected-node-label")).toContainText(
    "Selected out, outgoing port: root/greeting/port/outgoing/out",
  );
});

test("keeps faceplate controls focused while highlighting and updating source", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const input = page.locator('[data-id="greeting"] .control-input');
  const selectedSourceText = async () =>
    (await page.locator(".panel-source-selection").allTextContents()).join("");
  await input.click();

  await expect(input).toBeFocused();
  await expect.poll(selectedSourceText).toContain("node greeting");

  await input.fill("Edited on the faceplate.");
  await expect(input).toBeFocused();
  await expect(input).toHaveValue("Edited on the faceplate.");
  await expect(page.locator("#source")).toHaveValue(/Edited on the faceplate\./);
  await expect.poll(selectedSourceText).toContain("node greeting");
});

test("selects a cord by authoritative identity and reveals its declaration", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const edge = page.locator(".react-flow__edge").first();
  await edge.locator(".react-flow__edge-textbg").click();

  await expect(page.locator("#selected-node-label")).toContainText(
    "Selected cord: cord-0",
  );
  await expect(edge).toHaveClass(/selected/);
  const highlighted = (
    await page.locator(".panel-source-selection").allTextContents()
  ).join("");
  await expect(page.locator(".panel-source-selection")).toHaveCount(1);
  await expect(
    page.locator(".panel-source-selection .panel-token-keyword").filter({
      hasText: /^cord$/,
    }),
  ).toHaveCount(1);
  expect(highlighted).toContain("cord greeting.out -> output.in");
  expect(highlighted).toContain("pressure = block");
  const nativeSelection = await page.locator("#source").evaluate((element) =>
    element.value.slice(element.selectionStart, element.selectionEnd)
  );
  expect(nativeSelection).toBe(highlighted);
  const selectionStyle = await page.locator(".panel-source-selection").evaluate(
    (element) => ({
      backgroundColor: getComputedStyle(element).backgroundColor,
      outlineStyle: getComputedStyle(element).outlineStyle,
      outlineWidth: getComputedStyle(element).outlineWidth,
    }),
  );
  expect(selectionStyle).toEqual({
    backgroundColor: "rgba(56, 189, 248, 0.08)",
    outlineStyle: "solid",
    outlineWidth: "1px",
  });

  await page.locator('[data-id="greeting"]').click();
  await expect(page.locator("#selected-node-label")).toContainText(
    "Selected node: greeting",
  );
  await expect.poll(async () =>
    (await page.locator(".panel-source-selection").allTextContents()).join("")
  ).toContain("node greeting");
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
  const output = page.locator('[data-id="output"]');
  const outputBefore = await output.boundingBox();
  expect(outputBefore).not.toBeNull();
  const outputStartX = outputBefore.x + outputBefore.width / 2;
  const outputStartY = outputBefore.y + 20;
  await page.mouse.move(outputStartX, outputStartY);
  await page.mouse.down();
  await page.mouse.move(outputStartX - 72, outputStartY + 40, { steps: 4 });
  await page.mouse.up();
  const committedOutputTransform = await output.evaluate(
    (element) => element.style.transform,
  );

  await page.locator("#check").click();
  await expect(greeting).toHaveCSS("transform", /matrix/);
  await expect.poll(
    async () => greeting.evaluate((element) => element.style.transform),
  ).toBe(committedTransform);
  await expect.poll(
    async () => output.evaluate((element) => element.style.transform),
  ).toBe(committedOutputTransform);

  await page.locator("#run").click();
  await expect(page.locator("#result")).toContainText("Lesson complete", {
    timeout: 20_000,
  });
  await expect.poll(
    async () => greeting.evaluate((element) => element.style.transform),
  ).toBe(committedTransform);
  await expect.poll(
    async () => output.evaluate((element) => element.style.transform),
  ).toBe(committedOutputTransform);

  await page.getByRole("button", { name: "Inside / outside" }).click();
  await page.getByRole("button", { name: "Hello, panel" }).click();
  await expect.poll(
    async () => greeting.evaluate((element) => element.style.transform),
  ).toBe(committedTransform);
  await expect.poll(
    async () => output.evaluate((element) => element.style.transform),
  ).toBe(committedOutputTransform);

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
  await expect(path).toHaveAttribute("marker-end", /type=arrowclosed/);
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

test("typed text lesson shares format, lines, join, and ordered evidence", async ({ page }) => {
  await page.goto(
    "/tour/public/index.html?lesson=library.typed-text-format",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");
  const source = page.locator("#source");

  await expect(story).toBeVisible();
  await expect(story).toContainText("std/text/format");
  await expect(story).toContainText("std/format-values/literal");
  await expect(story).toContainText("std/text/lines");
  await expect(story).toContainText("std/text/join");
  await story.getByRole("button", { name: "std/text/format" }).click();
  await expect(page.locator("#selected-node-label")).toContainText("message");
  await expect(story.locator("#library-docs a")).toHaveCount(10);
  await expect(page.locator("#scenario option")).toHaveCount(6);
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

  await page.locator("#scenario").selectOption("lines-join");
  await page.locator("#run").click();
  await expect(result).toContainText("alpha | beta |  | gamma", { timeout: 20_000 });
  await expect(page.locator('[data-id="lines"]')).toContainText("std/text/lines");
  await expect(page.locator('[data-id="joined"]')).toContainText("std/text/join");

  await page.locator("#scenario").selectOption("format-lines");
  await page.locator("#run").click();
  await expect(result).toContainText("alpha / beta", { timeout: 20_000 });

  await page.locator("#scenario").selectOption("standalone");
  await source.fill((await source.inputValue()).replace("operator", "robot"));
  await page.locator("#run").click();
  await expect(result).toContainText("Hello, robot.", { timeout: 20_000 });
});

test("value envelope platform lesson links checked admission to an exact run", async ({ page }) => {
  await page.goto(
    "/tour/public/index.html?lesson=platform.value-envelope-clock-feedback",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");
  const source = page.locator("#source");

  await expect(story).toBeVisible();
  await expect(page.locator("#story-kind")).toHaveText("Platform contract lesson");
  await expect(page.locator("#scenario option")).toHaveCount(4);
  await expect(story).toContainText("bounded-envelope");
  await expect(story.locator("#library-docs a")).toHaveCount(3);

  await story.getByRole("button", { name: "cycle-without-boundary" }).click();
  await expect(result).toContainText("rejected before execution with CND-FBK-002");
  await expect(source).toHaveValue(/node emphasize : text\/uppercase/);

  await page.locator("#scenario").selectOption("finite-state-feedback");
  await expect(result).toContainText("admitted by the checked contract");
  await source.fill(
    (await source.inputValue()).replace(
      "Envelope facts stay exact.",
      "Edited envelope lesson.",
    ),
  );
  await page.locator("#run").click();
  await expect(result).toContainText("EDITED ENVELOPE LESSON.", {
    timeout: 20_000,
  });
  await expect(page.locator("#timeline-table tbody tr")).not.toHaveCount(0);
  await expect(page.locator("#timeline-table")).toContainText("block");
  await expect(page.locator("#timeline-table")).toContainText("succeeded");
  await expect(page.locator("#plan")).toContainText("bound-in-this-plan");
});

test("resource lease lesson keeps unknown commit and cleanup visible", async ({ page }) => {
  await page.goto(
    "/tour/public/index.html?lesson=platform.resource-lease-effect-commit",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");
  const source = page.locator("#source");

  await expect(story).toBeVisible();
  await expect(page.locator("#story-kind")).toHaveText("Platform contract lesson");
  await expect(page.locator("#scenario option")).toHaveCount(4);
  await expect(story).toContainText("lost-ack-is-commit-unknown");
  await expect(story.locator("#library-docs a")).toHaveCount(3);

  await story.getByRole("button", { name: "wrong-holder" }).click();
  await expect(result).toContainText("rejected before execution with CND-LSE-003");
  await expect(source).toHaveValue(/node emphasize : text\/uppercase/);

  await page.locator("#scenario").selectOption("lost-ack-is-commit-unknown");
  await expect(result).toContainText("admitted by the checked contract");
  await source.fill(
    (await source.inputValue()).replace(
      "Leased effect boundaries stay explicit.",
      "Edited lease lesson.",
    ),
  );
  await page.locator("#run").click();
  await expect(result).toContainText("EDITED LEASE LESSON.", {
    timeout: 20_000,
  });
  await expect(page.locator("#timeline-table tbody tr")).not.toHaveCount(0);
  await expect(page.locator("#timeline-table")).toContainText("succeeded");
});

test("workload lesson keeps hard admission distinct from observations", async ({ page }) => {
  await page.goto(
    "/tour/public/index.html?lesson=platform.workload-admission-deadline",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");
  const source = page.locator("#source");

  await expect(story).toBeVisible();
  await expect(page.locator("#story-kind")).toHaveText("Platform contract lesson");
  await expect(page.locator("#scenario option")).toHaveCount(5);
  await expect(story).toContainText("linux-measurement");
  await expect(story.locator("#library-docs a")).toHaveCount(3);

  await story.getByRole("button", { name: "unsupported-hard-real-time" }).click();
  await expect(result).toContainText("rejected before execution with CND-WRK-005");
  await expect(source).toHaveValue(/node emphasize : text\/uppercase/);

  await page.locator("#scenario").selectOption("browser-best-effort");
  await expect(result).toContainText("admitted by the checked contract");
  await source.fill(
    (await source.inputValue()).replace(
      "Deadline guarantees stay separate from measurements.",
      "Edited workload lesson.",
    ),
  );
  await page.locator("#run").click();
  await expect(result).toContainText("EDITED WORKLOAD LESSON.", {
    timeout: 20_000,
  });
  await expect(page.locator("#timeline-table tbody tr")).not.toHaveCount(0);
  await expect(page.locator("#timeline-table")).toContainText("succeeded");
});

test("cross-host lesson keeps discovery separate from exact provider binding", async ({ page }) => {
  await page.goto(
    "/tour/public/index.html?lesson=platform.cross-host-provider-conformance",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");
  const source = page.locator("#source");

  await expect(story).toBeVisible();
  await expect(page.locator("#story-kind")).toHaveText("Platform contract lesson");
  await expect(page.locator("#scenario option")).toHaveCount(6);
  await expect(story).toContainText("browser-wasm");
  await expect(story.locator("#library-docs a")).toHaveCount(3);

  await story.getByRole("button", { name: "firmware-unsupported" }).click();
  await expect(result).toContainText("rejected before execution with CND-HCF-005");
  await expect(source).toHaveValue(/node emphasize : text\/uppercase/);

  await page.locator("#scenario").selectOption("explicit-adapter");
  await expect(result).toContainText("admitted by the checked contract");
  await source.fill(
    (await source.inputValue()).replace(
      "Custom contracts need exact conformance and explicit adapters.",
      "Edited provider lesson.",
    ),
  );
  await page.locator("#run").click();
  await expect(result).toContainText("EDITED PROVIDER LESSON.", {
    timeout: 20_000,
  });
  await expect(page.locator("#timeline-table tbody tr")).not.toHaveCount(0);
  await expect(page.locator("#timeline-table")).toContainText("succeeded");
});
