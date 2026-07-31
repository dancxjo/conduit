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
    "conduit/hosted-literal",
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
  })).toHaveCount(1);
  await expect(highlight.locator(".panel-token-identifier").filter({
    hasText: /^output\.$/,
  })).toHaveCount(1);
  await expect(highlight.locator(".panel-token-port-outgoing")).toHaveText("value");
  await expect(highlight.locator(".panel-token-port-receiving")).toHaveText("text");
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
    "panel 0\n# note > ignored\ninterface speech/recognizer {\n" +
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
    "panel 0\n# note > ignored\ninterface speech/recognizer {\n" +
      "  > in : audio/pcm-stream\n" +
      "  in > : speech/transcript\n" +
      "  > audio : audio/pcm-stream\n" +
      "  committed > : speech/transcript\n" +
      "}\nnode value : fixture/source implements speech/recognizer\n",
  );

  await source.fill(
    "panel 0\ncomposite example/uppercase {\n" +
      "  node worker : text/uppercase\n" +
      "  export > text = worker.text\n" +
      "  export value < = worker.text\n" +
      "}\n",
  );
  await expect(highlight).toHaveAttribute("data-semantic-metadata", "available");
  await expect(highlight.locator(".panel-token-port-receiving")).toHaveText([
    "text",
    "text",
    "value",
    "text",
  ]);
  await expect(highlight.locator(".panel-token-port-sigil-receiving")).toHaveText([
    ">",
    "<",
  ]);
  await expect(
    highlight.locator(".panel-token-port-receiving").filter({ hasText: /^value$/ }),
  ).toHaveAttribute(
    "data-semantic-path",
    "definition/example/uppercase/port/receiving/value",
  );

  await source.fill('panel 0\ninterface broken {\n  > audio : "not > metadata"\n');
  await expect(highlight).toHaveAttribute("data-semantic-metadata", "unavailable");
  await expect(highlight.locator(".panel-token-port")).toHaveCount(0);
  await expect(highlight.locator(".panel-token-port-sigil")).toHaveCount(0);
  await expect(source).toHaveValue(
    'panel 0\ninterface broken {\n  > audio : "not > metadata"\n',
  );
  await expect(highlight).toHaveAttribute("aria-hidden", "true");
});

test("covers every published chapter and exposes production topology projections", async ({
  page,
}) => {
  await page.goto("/tour/public/index.html");
  const lessonCatalog = await page.evaluate(async () => {
    const response = await fetch("../lessons/current.json", { cache: "no-store" });
    const catalog = await response.json();
    return {
      count: catalog.lessons.length,
      chapters: [...new Set(catalog.lessons.map((lesson) => lesson.chapter))].sort(
        (left, right) => left - right,
      ),
    };
  });
  expect(lessonCatalog.chapters).toEqual(
    Array.from({ length: lessonCatalog.chapters.at(-1) + 1 }, (_, chapter) => chapter),
  );
  await expect(page.locator("#lessons > li")).toHaveCount(lessonCatalog.count);
  await page.getByRole("button", { name: "Inside / outside" }).click();
  await expect(page.locator("#source")).toHaveValue(/example\/upper-box/);
  await expect(page.locator("#logical-view")).toHaveAttribute("aria-pressed", "true");
  const logicalReceiving = page.locator("#panel-port-list").getByRole("button", {
    name: /box, text, receiving port, type std\/text/,
  });
  const logicalOutgoing = page.locator("#panel-port-list").getByRole("button", {
    name: /box, value, outgoing port, type std\/text/,
  });
  await expect(logicalReceiving).toContainText("box: > text");
  await expect(logicalOutgoing).toContainText("box: value >");
  await logicalReceiving.click();
  await expect(page.locator("#selected-node-label")).toContainText(
    "Selected text, receiving port: root/box/port/receiving/text",
  );
  await expect(page.locator(".panel-source-selection")).toHaveText("text");
  await page.locator("#expanded-view").click();
  await expect(page.locator("#logical-view")).toHaveAttribute("aria-pressed", "false");
  await expect(page.locator("#expanded-view")).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#topology")).toContainText(
    "box.worker : text/uppercase",
  );
  await expect(page.locator("#panel-port-list")).toContainText("box.worker: > text");
  await expect(page.locator("#panel-port-list")).toContainText("box.worker: text >");
  await page.locator("#logical-view").click();
  await expect(page.locator("#logical-view")).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#expanded-view")).toHaveAttribute("aria-pressed", "false");
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
      .replace("greeting.value", "salutation.value"),
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
  await expect(canvas).toHaveAttribute("data-projection", "rust-authoritative");
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
    name: "text, receiving port; type std/text",
    exact: true,
  });
  const outgoing = page.locator(".react-flow__node").getByRole("button", {
    name: "value, outgoing port; type std/text",
    exact: true,
  });
  await expect(receiving).toContainText("> text");
  await expect(outgoing).toContainText("value >");
  expect(
    await page.locator(".faceplate-port-row").allTextContents(),
  ).toEqual(expect.not.arrayContaining([expect.stringContaining("<")]));
  await expect(page.locator(".faceplate-type-compartment")).toHaveCount(2);
  await expect(page.locator(".faceplate-config-row")).toHaveCount(1);
  await expect(page.locator(".faceplate-config-row .jack-handle")).toHaveCount(0);
  await expect(page.locator(".faceplate-port-row")).toHaveCount(2);
  await expect(receiving.locator("..")).toHaveClass(/faceplate-port-row/);
  await expect(outgoing.locator("..")).toHaveClass(/faceplate-port-row/);
  for (const row of await page.locator(".faceplate-port-row").all()) {
    const rowBox = await row.boundingBox();
    const handleBox = await row.locator(".jack-handle").boundingBox();
    expect(rowBox).not.toBeNull();
    expect(handleBox).not.toBeNull();
    expect(Math.abs(
      (rowBox.y + rowBox.height / 2) -
      (handleBox.y + handleBox.height / 2),
    )).toBeLessThan(1);
  }
  await expect(page.locator("#panel-port-list")).toContainText("> text");
  await expect(page.locator("#panel-port-list")).toContainText("value >");
  await expect(page.locator("#panel-connection-list")).toContainText(
    "greeting.value > → > output.text",
  );
  await outgoing.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#selected-node-label")).toContainText(
    "Selected value, outgoing port: root/greeting/port/outgoing/value",
  );
  await expect(page.locator(".panel-source-selection")).toHaveText("value");
  const selectedEndpoint = await page.locator("#source").evaluate((element) =>
    element.value.slice(element.selectionStart, element.selectionEnd)
  );
  expect(selectedEndpoint).toBe("value");

  const greeting = page.locator('[data-id="greeting"]');
  await greeting.getByTitle("Collapse Faceplate").click();
  await expect(greeting.getByRole("button", {
    name: "value, outgoing port; type std/text",
    exact: true,
  })).toContainText("value >");
  const collapsedRow = greeting.locator(".faceplate-port-row");
  const collapsedHandle = collapsedRow.locator(".jack-handle");
  const collapsedRowBox = await collapsedRow.boundingBox();
  const collapsedHandleBox = await collapsedHandle.boundingBox();
  expect(Math.abs(
    (collapsedRowBox.y + collapsedRowBox.height / 2) -
    (collapsedHandleBox.y + collapsedHandleBox.height / 2),
  )).toBeLessThan(1);
  await greeting.getByTitle("Expand Faceplate").click();
});

test("draws bounded cords and exposes draggable rewire ends", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  await source.fill(
    "panel 0\n\n" +
    "node first : std/literal { value = \"first\" }\n" +
    "node primary : display/text\n",
  );
  await expect(page.locator(".react-flow__node")).toHaveCount(2);

  const dragHandle = async (from, to) => {
    await expect(from).toBeVisible();
    await expect(to).toBeVisible();
    const fromBox = await from.boundingBox();
    const toBox = await to.boundingBox();
    expect(fromBox).not.toBeNull();
    expect(toBox).not.toBeNull();
    await from.hover();
    await page.mouse.down();
    await page.mouse.move(
      fromBox.x + fromBox.width / 2 + 1,
      fromBox.y + fromBox.height / 2,
    );
    await page.mouse.move(
      toBox.x + toBox.width / 2,
      toBox.y + toBox.height / 2,
      { steps: 8 },
    );
    await to.hover();
    await page.mouse.up();
  };
  const handle = (nodeId) => page.locator(
    `.react-flow__node[data-id="${nodeId}"] .jack-handle`,
  );

  await dragHandle(handle("first"), handle("primary"));
  await expect(page.locator(".react-flow__edge")).toHaveCount(1);
  await expect(source).toHaveValue(/cord first\.value -> primary\.text/);
  await expect(source).toHaveValue(/max_queued_bytes = 1024/);

  await page.locator(".react-flow__edge-textbg").click();
  const updaters = page.locator(
    ".react-flow__edge.selected .react-flow__edgeupdater",
  );
  await expect(updaters).toHaveCount(2);
  await expect(updaters.first()).toHaveCSS("pointer-events", "all");
});

test("renders composite exports as public faceplate ports", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.getByRole("button", { name: "Inside / outside" }).click();
  const composite = page.locator(".composite-faceplate").first();
  await expect(composite).toContainText("public");
  await expect(composite.locator(".public-jack-handle")).toHaveCount(2);
  await expect(
    composite.locator(".faceplate-port-row .jack-status-dot.connected"),
  ).toHaveCount(2);
  await expect(page.locator(".react-flow__edge")).toHaveCount(2);
  const publicJack = await composite.locator(".public-jack-handle").first().boundingBox();
  const internalJack = await page.locator(
    ".conduit-faceplate-card:not(.composite-faceplate) .jack-handle",
  ).first().boundingBox();
  expect(publicJack.width).toBeGreaterThan(internalJack.width);
  await page.locator("#expanded-view").click();
  await expect(page.locator(".composite-faceplate")).toHaveCount(0);
  await expect(page.locator('.react-flow__node[data-id="box.worker"]')).toHaveCount(1);
  await page.locator(".react-flow__edge").first()
    .locator(".react-flow__edge-textbg").click();
  await expect(page.locator(".faceplate-port-row.selected-cord-endpoint")).toHaveCount(2);
});

test("keeps semantic port direction redundant across presentation media", async ({ page }) => {
  await page.emulateMedia({
    colorScheme: "light",
    forcedColors: "active",
    reducedMotion: "reduce",
  });
  await page.goto("/tour/public/index.html");
  const receiving = page.locator(".react-flow__node").getByRole("button", {
    name: "text, receiving port; type std/text",
    exact: true,
  });
  const outgoing = page.locator(".react-flow__node").getByRole("button", {
    name: "value, outgoing port; type std/text",
    exact: true,
  });
  await expect(receiving).toContainText("> text");
  await expect(outgoing).toContainText("value >");
  await expect(receiving.locator("..")).toHaveAttribute(
    "data-port-direction",
    "receiving",
  );
  await expect(outgoing.locator("..")).toHaveAttribute(
    "data-port-direction",
    "outgoing",
  );
  await expect(page.locator(".patchbay-smart-cord").first()).toHaveCSS(
    "animation-name",
    "none",
  );

  await page.evaluate(() => {
    document.documentElement.style.zoom = "200%";
  });
  await expect(receiving).toBeVisible();
  await expect(outgoing).toBeVisible();

  await page.emulateMedia({
    colorScheme: "dark",
    forcedColors: "none",
    reducedMotion: "no-preference",
  });
  await expect(receiving).toContainText("> text");
  await expect(outgoing).toContainText("value >");
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
  await expect(page.locator(".faceplate-port-row.selected-cord-endpoint")).toHaveCount(2);
  await expect(page.locator(".panel-source-endpoint")).toHaveCount(2);
  expect(await page.locator(".panel-source-endpoint").allTextContents()).toEqual([
    "value",
    "text",
  ]);
  const highlighted = (
    await page.locator(".panel-source-selection").allTextContents()
  ).join("");
  await expect(page.locator(".panel-source-selection")).toHaveCount(1);
  await expect(
    page.locator(".panel-source-selection .panel-token-keyword").filter({
      hasText: /^cord$/,
    }),
  ).toHaveCount(1);
  expect(highlighted).toContain("cord greeting.value -> output.text");
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

test("renders the direction lesson as an invalid authored graph", async ({ page }) => {
  await page.goto("/tour/public/index.html?lesson=nodes.direction-matters");

  await expect(page.locator('.react-flow__node[data-id="first"]')).toBeVisible();
  await expect(page.locator('.react-flow__node[data-id="second"]')).toBeVisible();
  await expect(
    page.locator('[data-id="second"] [data-port-direction="outgoing"]'),
  ).toContainText("value >");
  const edge = page.locator(".patchbay-smart-cord");
  await expect(edge).toHaveCount(1);
  await expect(edge).toHaveClass(/cord-diagnostic-error/);
  await expect(edge).toHaveClass(/cord-validity-wrong-direction/);
  await expect(edge.locator(".react-flow__edge-path")).toHaveCSS(
    "stroke",
    "rgb(255, 23, 68)",
  );
  await expect(edge.locator(".react-flow__edge-text")).toContainText(
    "× wrong direction ×",
  );
  await expect(page.locator(".diagnostic-anchor-row")).toContainText(
    "second.value",
  );
  const diagnostic = page.locator("#diagnostic-console").getByRole("button", {
    name: /CND-CMP-003/,
  });
  await expect(diagnostic).toContainText(
    "Outgoing port used as destination",
  );
  await diagnostic.click();
  await expect(page.locator("#result")).toContainText(
    "a cord must terminate at a receiving port",
  );
  await expect(page.locator("#selected-node-label")).toContainText(
    "Selected cord: cord-0",
  );
  await expect(page.locator(".faceplate-port-row.selected-cord-endpoint")).toHaveCount(2);
  await expect(page.locator(".panel-source-selection")).toContainText(
    "cord first.value -> second.value",
  );
  await expect(page.locator("#plan")).toContainText(
    "No Rust-resolved plan for this source yet.",
  );
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#evidence")).not.toContainText('"event_kind"');
});

test("keeps invalid, unresolved, incomplete, and corrected revisions distinct", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/node greeting/, { timeout: 20_000 });
  const original = await source.inputValue();
  const destinationInvalid = "panel 0\n" +
    "node greeting : std/literal { value = \"invalid\" }\n" +
    "node output : display/text\n" +
    "cord greeting.value -> greeting.value\n";
  await source.fill(destinationInvalid);
  await expect(source).toHaveValue(/greeting\.value -> greeting\.value/);
  await expect(page.locator(".patchbay-smart-cord")).toHaveClass(
    /cord-validity-wrong-direction/,
    { timeout: 20_000 },
  );
  await expect(page.locator("#run")).toBeDisabled();

  const incomplete = `${original}\nnode provisional :`;
  await source.fill(incomplete);
  await expect(page.locator('[data-id="greeting"]')).toBeVisible();
  await expect(page.locator('[data-id="output"]')).toBeVisible();
  await expect(page.locator('[data-id="provisional"]')).toHaveClass(
    /react-flow__node/,
  );
  await expect(
    page.locator('[data-id="provisional"] .conduit-faceplate-card'),
  ).toHaveClass(/faceplate-validity-incomplete/, { timeout: 20_000 });
  await expect(page.locator("#run")).toBeDisabled();

  await source.fill(`${original}\nnode provisional : missing/contract\n`);
  await expect(
    page.locator('[data-id="provisional"] .conduit-faceplate-card'),
  ).toHaveClass(/faceplate-validity-unresolved/, { timeout: 20_000 });
  await expect(page.locator("#diagnostic-console")).toContainText(
    "No ports, provider, placement, or plan are inferred",
  );

  await source.fill(original);
  await expect(page.locator(".patchbay-smart-cord")).toHaveClass(
    /cord-validity-valid/,
    { timeout: 20_000 },
  );
  await expect(page.locator('[data-id="greeting"]')).toBeVisible();
  await expect(page.locator("#run")).toBeEnabled();
});

test("projects every authored cord failure family with static reduced-motion cues", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/node greeting/, { timeout: 20_000 });
  const cases = [
    {
      state: "wrong-direction",
      panel: "panel 0\nnode a : display/text\nnode b : display/text\ncord a.text -> b.text\n",
    },
    {
      state: "unresolved",
      panel: "panel 0\nnode b : display/text\ncord missing.value -> b.text\n",
    },
    {
      state: "incompatible",
      panel: "panel 0\nnode a : std/literal\nnode b : io/stdout\ncord a.value -> b.bytes\n",
    },
    {
      state: "invalid-bounds",
      panel: "panel 0\nnode a : std/literal\nnode b : display/text\n" +
        "cord a.value -> b.text { capacity = 1 max_value_bytes = 8 " +
        "max_queued_bytes = 8 low_watermark = 0 high_watermark = 2 pressure = block }\n",
    },
  ];
  for (const fixture of cases) {
    await source.fill(fixture.panel);
    await expect(source).toHaveValue(fixture.panel);
    const edge = page.locator(".patchbay-smart-cord");
    await expect(edge).toHaveClass(
      new RegExp(`cord-validity-${fixture.state}`),
      { timeout: 20_000 },
    );
    await expect(edge.locator(".react-flow__edge-text")).toContainText("×");
    await expect(edge.locator(".react-flow__edge-path")).toHaveCSS(
      "animation-name",
      "none",
    );
    const dash = await edge.locator(".react-flow__edge-path").evaluate(
      (element) => getComputedStyle(element).strokeDasharray,
    );
    expect(dash).not.toBe("none");
    await expect(page.locator("#run")).toBeDisabled();
  }
});

test("emphasizes one of several diagnostics without replaying unchanged checks", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/node greeting/, { timeout: 20_000 });
  await source.fill(
    "panel 0\n" +
    "node a : std/literal\n" +
    "node b : std/literal\n" +
    "node c : std/literal\n" +
    "cord a.value -> b.value\n" +
    "cord b.value -> c.value\n",
  );
  const edges = page.locator(".patchbay-smart-cord");
  await expect(edges).toHaveCount(2);
  await expect(edges.filter({ has: page.locator(".react-flow__edge-path") })).toHaveCount(2);
  await expect(page.locator(".patchbay-smart-cord.diagnostic-emphasized")).toHaveCount(1);
  await expect(
    page.locator(".patchbay-smart-cord:not(.diagnostic-emphasized)")
      .locator(".react-flow__edge-path"),
  ).toHaveCSS("animation-name", "none");

  const emphasizedPath = page.locator(
    ".patchbay-smart-cord.diagnostic-emphasized .react-flow__edge-path",
  );
  await page.waitForTimeout(250);
  const before = await emphasizedPath.evaluate((element) => ({
    currentTime: element.getAnimations()[0]?.currentTime ?? 0,
    geometry: element.getAttribute("d"),
  }));
  await page.locator("#check").click();
  await page.waitForTimeout(120);
  const after = await emphasizedPath.evaluate((element) => ({
    currentTime: element.getAnimations()[0]?.currentTime ?? 0,
    geometry: element.getAttribute("d"),
  }));
  expect(after.currentTime).toBeGreaterThan(before.currentTime);
  expect(after.geometry).toBe(before.geometry);
  await expect(page.locator("#diagnostic-console li")).toHaveCount(2);
});

test("routes cords through free space and keeps labels off node faces", async ({ page }) => {
  await page.addInitScript(() => {
    window.CONDUIT_PATCHBAY_FEATURES = { legacyLinePlacement: true };
  });
  await page.goto("/tour/public/index.html");

  const panelSource = "panel 0\n\n" +
    "node source : std/literal {\n" +
    "  value = \"source\"\n" +
    "}\n" +
    "node transform : text/uppercase\n" +
    "node sink : display/text\n\n" +
    "cord source.value -> transform.text {\n" +
    "  capacity = 1\n" +
    "  max_value_bytes = 1024\n" +
    "  max_queued_bytes = 1024\n" +
    "  low_watermark = 0\n" +
    "  high_watermark = 1\n" +
    "  pressure = block\n" +
    "}\n\n" +
    "cord transform.text -> sink.text {\n" +
    "  capacity = 1\n" +
    "  max_value_bytes = 1024\n" +
    "  max_queued_bytes = 1024\n" +
    "  low_watermark = 0\n" +
    "  high_watermark = 1\n" +
    "  pressure = block\n" +
    "}\n";
  const source = page.locator("#source");
  await expect(page.locator(".react-flow__edge")).toHaveCount(1);
  await source.fill(panelSource);
  await expect(page.locator(".react-flow__edge")).toHaveCount(2);

  const flow = page.locator("#cy");
  const flowBox = await flow.boundingBox();
  expect(flowBox).not.toBeNull();

  const dragNodeTo = async (nodeId, absoluteX, absoluteY) => {
    const node = page.locator(`.react-flow__node[data-id="${nodeId}"]`);
    await expect(node).toHaveCount(1);
    const nodeBox = await node.boundingBox();
    expect(nodeBox).not.toBeNull();
    await page.mouse.move(
      nodeBox.x + nodeBox.width / 2,
      nodeBox.y + nodeBox.height / 2,
    );
    await page.mouse.down();
    await page.mouse.move(absoluteX, absoluteY, { steps: 8 });
    await page.mouse.up();
  };

  await dragNodeTo(
    "source",
    flowBox.x + flowBox.width / 2,
    flowBox.y + 190,
  );
  await dragNodeTo(
    "transform",
    flowBox.x + flowBox.width / 2,
    flowBox.y + 60,
  );
  await dragNodeTo(
    "sink",
    flowBox.x + flowBox.width / 2,
    flowBox.y + 320,
  );

  const edge = page.locator(".patchbay-smart-cord").nth(1);
  await expect(edge).toHaveCount(1);
  await expect
    .poll(async () => edge.locator(".react-flow__edge-path").getAttribute("d"))
    .not.toBe("");

  const hasCollision = await edge.evaluate((edgeElement, clearance) => {
    const path = edgeElement.querySelector(".react-flow__edge-path");
    if (!path) return false;
    const totalLength = path.getTotalLength();
    if (!Number.isFinite(totalLength) || totalLength <= 0) return false;
    const sampleCount = 240;
    const endpointIds = new Set([
      path.dataset.sourceNode,
      path.dataset.targetNode,
    ].filter(Boolean));
    const nodes = Array.from(document.querySelectorAll(".react-flow__node"))
      .filter((node) => !endpointIds.has(node.dataset.id))
      .map((node) => {
        const bounds = node.getBoundingClientRect();
        return {
          left: bounds.left - clearance,
          right: bounds.right + clearance,
          top: bounds.top - clearance,
          bottom: bounds.bottom + clearance,
        };
      });
    for (let index = 0; index <= sampleCount; index += 1) {
      const ratio = index / sampleCount;
      if (ratio < 0.05 || ratio > 0.95) continue;
      const point = path.getPointAtLength(totalLength * ratio);
      const screenPoint = point.matrixTransform(path.getScreenCTM());
      const hits = nodes.some((bounds) =>
        screenPoint.x > bounds.left &&
        screenPoint.x < bounds.right &&
        screenPoint.y > bounds.top &&
        screenPoint.y < bounds.bottom,
      );
      if (hits) {
        return true;
      }
    }
    return false;
  }, 12);
  expect(hasCollision).toBe(false);

  const labelCollides = await edge.evaluate((edgeElement, clearance) => {
    const label = edgeElement.querySelector(".react-flow__edge-textbg");
    if (!label) return false;
    const rect = label.getBoundingClientRect();
    return Array.from(document.querySelectorAll(".react-flow__node")).some((node) => {
      const bounds = node.getBoundingClientRect();
      rect.left < bounds.right + clearance &&
        rect.right > bounds.left - clearance &&
        rect.top < bounds.bottom + clearance &&
        rect.bottom > bounds.top - clearance;
    });
  }, 6);
  expect(labelCollides).toBe(false);

  await dragNodeTo("transform", flowBox.x + 80, flowBox.y + 70);
  await expect.poll(async () => {
    return edge.evaluate((edgeElement, clearance) => {
      const path = edgeElement.querySelector(".react-flow__edge-path");
      if (!path) return false;
      const totalLength = path.getTotalLength();
      if (!Number.isFinite(totalLength) || totalLength <= 0) return false;
      const sampleCount = 280;
      const endpointIds = new Set([
        path.dataset.sourceNode,
        path.dataset.targetNode,
      ].filter(Boolean));
      const nodes = Array.from(document.querySelectorAll(".react-flow__node"))
        .filter((node) => !endpointIds.has(node.dataset.id))
        .map((node) => {
          const bounds = node.getBoundingClientRect();
          return {
            left: bounds.left - clearance,
            right: bounds.right + clearance,
            top: bounds.top - clearance,
            bottom: bounds.bottom + clearance,
          };
        });
      for (let index = 0; index <= sampleCount; index += 1) {
        const ratio = index / sampleCount;
        if (ratio < 0.05 || ratio > 0.95) continue;
        const point = path.getPointAtLength(totalLength * ratio);
        const screenPoint = point.matrixTransform(path.getScreenCTM());
        const hits = nodes.some((bounds) =>
          screenPoint.x > bounds.left &&
          screenPoint.x < bounds.right &&
          screenPoint.y > bounds.top &&
          screenPoint.y < bounds.bottom,
        );
        if (hits) {
          return true;
        }
      }
      return false;
    }, 12);
  }).toBe(false);
});

test("filesystem reference panels use the explicit bounded browser provider", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.getByRole("button", { name: "File Copier Pipeline" }).click();
  await expect(page.locator("#runnability-state")).toContainText("runnable · browser");
  await expect(page.locator("#run")).toBeEnabled();
  await expect(page.locator("#source")).toHaveValue(/node reader : fs\/read/);
  await page.locator("#run").click();
  await expect(page.locator("#result")).toContainText("Run completed", {
    timeout: 20_000,
  });
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

test("multi-port lesson runs its explicit display composite", async ({ page }) => {
  await page.goto("/tour/public/index.html");
  await page.getByRole("button", { name: "More than one port" }).click();
  await expect(page.locator("#runnability-state")).toContainText(
    "runnable · browser",
  );
  await expect(page.locator("#run")).toBeEnabled();
  await page.locator("#run").click();
  await expect(page.locator("#result")).toContainText("Left.\nRight.", {
    timeout: 20_000,
  });
  await expect(page.locator("#evidence")).toContainText('"event_kind": "terminal"');
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
    'Exact display: "Hello, operator.\\n"',
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
