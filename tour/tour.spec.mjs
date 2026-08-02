import { expect, test } from "@playwright/test";

function parseWatchTick(text) {
  const scoped = /tick=(\d+)/.exec(text || "");
  return scoped ? Number.parseInt(scoped[1], 10) : Number.parseInt(text, 10);
}

async function replaceSourceText(source, before, after) {
  await source.evaluate((element, replacement) => {
    element.value = element.value.replace(replacement.before, replacement.after);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  }, { before, after });
}

async function gotoTour(page, path) {
  await page.goto(path);
  await expect(page.locator("html")).toHaveAttribute(
    "data-tour-ready",
    "true",
    { timeout: 20_000 },
  );
}

function collectPageFailures(page) {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });
  return failures;
}

async function openTinyInstrument(page) {
  await gotoTour(page, "/tour/public/index.html?lesson=panels.tiny-instrument");
  await page.locator("#accounting-drawer > summary").click();
  await expect(page.locator("#run")).toBeEnabled({ timeout: 20_000 });
}

async function startTinyInstrument(page) {
  await openTinyInstrument(page);
  await page.locator("#run").click();
  await expect.poll(async () => {
    const text = await page.locator("#watch-value").textContent();
    return parseWatchTick(text);
  }, { timeout: 20_000 }).toBeGreaterThanOrEqual(0);
  const firstTick = parseWatchTick(
    await page.locator("#watch-value").textContent(),
  );
  const first = await page.locator("#watch-accounting").evaluate((element) =>
    JSON.parse(element.textContent)
  );
  const browserPlan = await page.evaluate(() =>
    fetch("/tour/public/browser-plan.json", { cache: "no-store" })
      .then((response) => response.json())
  );
  return { browserPlan, first, firstTick };
}

async function dragAndCommitTopologyNode(page, node, deltaX, deltaY) {
  await node.scrollIntoViewIfNeeded();
  const before = await node.boundingBox();
  expect(before).not.toBeNull();
  const beforeTransform = await node.evaluate((element) => element.style.transform);
  const startX = before.x + before.width / 2;
  const startY = before.y + 20;
  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.mouse.move(startX + deltaX, startY + deltaY, { steps: 4 });
  await page.mouse.up();
  await expect.poll(
    async () => node.evaluate((element) => element.style.transform),
  ).not.toBe(beforeTransform);
  return node.evaluate((element) => element.style.transform);
}

async function clickCordPath(page, edge) {
  await edge.scrollIntoViewIfNeeded();
  const point = await edge.locator(".react-flow__edge-interaction").evaluate((path) => {
    const matrix = path.getScreenCTM();
    if (!matrix) throw new Error("cord interaction path has no screen transform");
    const local = path.getPointAtLength(path.getTotalLength() / 4);
    const screen = new DOMPoint(local.x, local.y).matrixTransform(matrix);
    return { x: screen.x, y: screen.y };
  });
  const hit = await page.evaluate(({ x, y }) => {
    const target = document.elementFromPoint(x, y);
    return {
      tag: target?.tagName || "",
      className: typeof target?.className === "string"
        ? target.className
        : target?.className?.baseVal || "",
    };
  }, point);
  expect(hit.tag).toBe("path");
  expect(hit.className).toMatch(/react-flow__edge-(?:interaction|path)/);
  await page.mouse.click(point.x, point.y);
}

async function openTypedTextLesson(page) {
  await gotoTour(page, "/tour/public/index.html?lesson=library.typed-text-format");
  const story = page.locator("#execution-story");
  await expect(story).toBeVisible();
  return {
    result: page.locator("#result"),
    source: page.locator("#source"),
    story,
  };
}

test("runs a production lesson in the resolved browser worker", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });

  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel&autorun");
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

test("opens as a book and embeds the same real lab in compact and expanded modes", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html");

  await expect(page.locator("#book-cover")).toBeVisible();
  await expect(page.locator("#cover-title")).toHaveText(
    "Build a living system you can see",
  );
  await expect(page.locator("#cover-projects article")).toHaveCount(4);
  await expect(page.locator("#workspace")).toBeHidden();

  await page.locator("#begin-book").click();
  await expect(page.locator("#reader-section-title")).toHaveText(
    "Recover the hidden program",
  );
  await expect(page.locator("#artifact-id")).toHaveText("origin-map");
  await expect(page.locator("#opening-result")).toContainText(
    "Toggle from recognizable products",
  );
  await expect(page.locator("#chapter-opening-title")).toHaveText(
    "The program we could no longer see",
  );
  await expect(page.locator("#narrative-before-lab .narrative-block")).toHaveCount(4);
  await expect(page.locator("#narrative-after-lab .narrative-block")).toHaveCount(4);
  await expect(page.locator("#workspace")).toHaveAttribute("data-mode", "compact");
  await expect(page.locator("#source")).toHaveValue(/time\/ticker/);
  await expect(page.locator("#plan-drawer")).not.toHaveAttribute("open", "");
  await expect(page.locator("#evidence-drawer")).not.toHaveAttribute("open", "");

  const sourceBeforeExpansion = await page.locator("#source").inputValue();
  await page.locator("#expand-lab").click();
  await expect(page.locator("#workspace")).toHaveAttribute("data-mode", "expanded");
  await expect(page.locator("#source")).toHaveValue(sourceBeforeExpansion);
  await expect(page.locator("#expand-lab")).toHaveAttribute("aria-expanded", "true");

  await page.locator("#next-section").click();
  await expect(page.locator("#reader-section-title")).toHaveText("Wake the instrument");
  await expect(page.locator("#previous-section")).toContainText(
    "Previous: Recover the hidden program",
  );
  await expect(page.locator("#artifact-id")).toHaveText("living-instrument.panel");
});

test("keeps Reference and Cookbook searchable outside sequential navigation", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html");

  await page.locator("#show-cookbook").click();
  await expect(page.locator("#directory-title")).toHaveText("Cookbook");
  await page.locator("#directory-query").fill("codec");
  await expect(page.locator("#directory-results")).toContainText(
    "Use exact PCM and WAVE operations",
  );
  await page.getByRole("button", { name: /Use exact PCM and WAVE operations/ }).click();
  await expect(page.locator("#section-progress")).toHaveText(
    "Outside sequential book progress",
  );
  await expect(page.locator("#previous-section")).toHaveText("Back to Cookbook");
  await expect(page.locator("#next-section")).toBeHidden();

  await page.locator("#show-reference").click();
  await expect(page.locator("#directory-title")).toHaveText("Reference");
  await page.locator("#directory-query").fill("filesystem");
  await expect(page.locator("#directory-results")).toContainText("File Copier Pipeline");
});

test("restores reading position and a local draft without reviving a run", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?section=instrument.wake");
  await page.locator("#expand-lab").click();
  const source = page.locator("#source");
  await source.scrollIntoViewIfNeeded();
  await replaceSourceText(source, "duration_ticks = 1000", "duration_ticks = 1200");
  await page.locator("#reader-content").evaluate((reader) => {
    reader.scrollTop = 420;
    reader.dispatchEvent(new Event("scroll"));
  });
  await expect.poll(async () => page.locator("#reader-content").evaluate(
    (reader) => reader.scrollTop,
  )).toBeGreaterThan(0);
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute(
    "data-tour-ready",
    "true",
    { timeout: 20_000 },
  );

  await expect(page.locator("#reader-section-title")).toHaveText("Wake the instrument");
  await expect(source).toHaveValue(/duration_ticks = 1200/);
  await expect(page.locator("#console-status-badge")).toHaveText("Ready");
  await expect(page.locator("#result")).not.toContainText("Live exact run remains");
  await expect(page.locator("#artifact-status")).toContainText(
    "not a live-run claim",
  );
  await expect.poll(async () => page.locator("#reader-content").evaluate(
    (reader) => reader.scrollTop,
  )).toBeGreaterThan(0);
});

test("carries and resets cumulative project state explicitly", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?section=instrument.wake");
  const source = page.locator("#source");
  await replaceSourceText(source, "duration_ticks = 1000", "duration_ticks = 1400");
  await page.locator("#next-section").click();

  await expect(page.locator("#reader-section-title")).toHaveText("Give it a heartbeat");
  await expect(page.locator("#artifact-status")).toContainText("instrument-running");
  await page.locator("#reset-project").dispatchEvent("click");
  await expect(page.locator("#reader-section-title")).toHaveText("Wake the instrument");
  await expect(page.locator("#artifact-status")).toContainText("instrument-ready");
  await expect(source).toHaveValue(/duration_ticks = 1000/);
  await expect(page.locator("#recover-project")).toBeEnabled();
});

test("recovers cumulative project drafts explicitly", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?section=instrument.wake");
  const source = page.locator("#source");
  await replaceSourceText(source, "duration_ticks = 1000", "duration_ticks = 1400");
  await page.locator("#reset-project").dispatchEvent("click");
  await expect(page.locator("#artifact-status")).toContainText("instrument-ready");
  await expect(source).toHaveValue(/duration_ticks = 1000/);
  await expect(page.locator("#recover-project")).toBeEnabled();
  await page.locator("#recover-project").dispatchEvent("click");
  await expect(page.locator("#artifact-status")).toContainText("instrument-ready");
  await expect(source).toHaveValue(/duration_ticks = 1400/);
  await expect(page.locator("#recover-project")).toBeDisabled();
});

for (const [section, nextTitle, sourcePattern] of [
  ["instrument.wake", "Give it a heartbeat", /conduit\.media\/control\/sequencer/],
  ["service.listen", "Waiting is not completion", /server: net\/http\/listen/],
  ["robot.rehearse", "Choose hosts without changing meaning", /wifi_ap: net\/wifi\/access-point/],
]) {
  test(`keeps one canonical source artifact through the ${section} build`, async ({ page }) => {
    await gotoTour(page, `/tour/public/index.html?section=${section}`);
    const source = page.locator("#source");
    await expect(source).toHaveValue(sourcePattern);
    const before = await source.inputValue();
    await page.locator("#next-section").click();
    await expect(page.locator("#reader-section-title")).toHaveText(nextTitle);
    await expect(source).toHaveValue(before);
    await expect(page.locator("#artifact-status")).toContainText(
      "This is reader state, not a live-run claim",
    );
  });
}

test("keeps the project path keyboard-operable with adjacent reduced-motion equivalents", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await gotoTour(page, "/tour/public/index.html");
  await page.locator("#begin-book").focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#reader-section-title")).toHaveText(
    "Recover the hidden program",
  );
  await expect(page.locator("#accessibility-equivalent")).toHaveAttribute("open", "");
  await expect(page.locator("#section-non-audio")).not.toHaveText("");
  await expect(page.locator("#section-reduced-motion")).not.toHaveText("");
  await expect(page.locator("#section-screen-reader")).not.toHaveText("");
  await expect(page.locator("#reader-content")).toHaveCSS("scroll-behavior", "auto");
  await expect(page.locator("#plan-drawer")).not.toHaveAttribute("open", "");

  await page.locator("#next-section").focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#reader-section-title")).toHaveText("Wake the instrument");
  await expect(page.locator("#artifact-status")).toContainText(
    "This is reader state, not a live-run claim",
  );
  await expect(page.locator("#plan-drawer")).not.toHaveAttribute("open", "");
});

test("routes Book, Cookbook, Reference and retired lesson links through the migration ledger", async ({
  page,
}) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  await expect(page.locator("#reader-section-title")).toHaveText("Retired: Hello, panel");
  await expect(page.locator("#section-progress")).toContainText(
    "exact fixture retained",
  );
  await page.locator("#previous-section").click();
  await expect(page.locator("#reader-section-title")).toHaveText("Wake the instrument");

  await gotoTour(page, "/tour/public/index.html?lesson=library.bounded-http-service");
  await expect(page.locator("#reader-section-title")).toHaveText(
    "Open the service boundary",
  );
  await expect(page.locator("#artifact-id")).toHaveText("bounded-service.panel");
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#result")).toContainText("CND-IMP-001");

  await gotoTour(page, "/tour/public/index.html?lesson=nodes.more-than-one-port");
  await expect(page.locator("#project-progress")).toHaveText("Reference lesson");
  await expect(page.locator("#project-artifact")).toBeHidden();

  await gotoTour(page, "/tour/public/index.html?lesson=library.bounded-media-codecs");
  await expect(page.locator("#project-progress")).toHaveText("Cookbook recipe");
  await expect(page.locator("#reader-section-title")).toHaveText(
    "Use exact PCM and WAVE operations",
  );
});

test("keeps prose, action, real lab, result and explanation in document order", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?section=why.visible-program#plan-drawer");
  await expect(page.locator("#plan-drawer")).toHaveAttribute("open", "");
  const order = await page.evaluate(() => {
    const nodes = [
      document.querySelector("#narrative-before-lab"),
      document.querySelector("#run"),
      document.querySelector("#result"),
      document.querySelector("#narrative-after-lab"),
    ];
    return nodes.map((node) => {
      let index = 0;
      for (const candidate of document.querySelectorAll("body *")) {
        if (candidate === node) return index;
        index += 1;
      }
      return -1;
    });
  });
  expect(order).toEqual([...order].sort((left, right) => left - right));
  await expect(page.locator("#section-permalink")).toHaveAttribute(
    "href",
    "?section=why.visible-program",
  );
});

test("owns an exact Patchbay run session inside the dedicated worker", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");

  const result = await page.evaluate(async () => {
    const plan = await fetch("/tour/public/browser-plan.json", { cache: "no-store" })
      .then((response) => response.json());
    const wasm = plan.artifacts.find((artifact) => artifact.id === "conduit-web-wasm");
    const worker = new Worker("/tour/public/tour-worker.mjs", { type: "module" });
    let nextId = 1;
    const request = (operation, value) => new Promise((resolve, reject) => {
      const id = nextId++;
      const timeout = setTimeout(() => reject(new Error(`worker timeout: ${operation}`)), 10_000);
      const receive = (event) => {
        if (event.data?.id !== id) return;
        clearTimeout(timeout);
        worker.removeEventListener("message", receive);
        resolve(event.data);
      };
      worker.addEventListener("message", receive);
      worker.postMessage({ id, operation, value });
    });

    try {
      const configured = await request("configure", {
        wasmUrl: new URL(wasm.path, location.href).href,
        wasmSha256: wasm.sha256,
      });
      const source = "panel 0\ngreeting: std/literal { value = \"hello\\n\" }\n" +
        "output: display/text\ngreeting.value > output.text\n";
      const opened = await request("patchbay-open-session", {
        documentId: "tour/worker-exact-session",
        source,
      });
      const sessionId = opened.value?.session_id;
      const started = await request("patchbay-start-exact-run", { sessionId });
      const runIdentity = {
        runId: started.value?.run_id,
        sourceRevision: started.value?.source_revision,
        planIdentity: started.value?.plan_identity,
      };
      const watchAdmission = started.value?.view?.plan?.watch_admissions?.[0];
      const watchId = watchAdmission?.id;
      const operatorId = watchAdmission?.operator;
      const attachedWatch = await request("patchbay-attach-exact-watch", {
        sessionId,
        ...runIdentity,
        watchId,
        operatorId,
      });
      const pumped = await request("patchbay-pump-exact-run", {
        sessionId,
        ...runIdentity,
        quantum: 1,
      });
      const watch = await request("patchbay-read-exact-watch", {
        sessionId,
        ...runIdentity,
        watchId,
        operatorId,
        cursor: 0,
        maximumRecords: 1,
      });
      const detachedWatch = await request("patchbay-detach-exact-watch", {
        sessionId,
        ...runIdentity,
        watchId,
        operatorId,
      });
      const evidence = await request("patchbay-read-exact-evidence", {
        sessionId,
        ...runIdentity,
        cursor: 0,
        maximumEvents: 1,
      });
      const repeatedEvidence = await request("patchbay-read-exact-evidence", {
        sessionId,
        ...runIdentity,
        cursor: 0,
        maximumEvents: 1,
      });
      const invalidCandidate = await request("patchbay-apply-transaction", {
        sessionId,
        requestJson: JSON.stringify({
          protocol_version: 0,
          document_id: sessionId,
          expected_source_revision: 0,
          expected_presentation_revision: 0,
          operations: [{ ReplaceSource: { source: "panel 0\nbroken :" } }],
        }),
      });
      const activeAfterCandidate = await request("patchbay-session-view", { sessionId });
      const malformed = await request("patchbay-notify-host-operation", {
        sessionId,
        ...runIdentity,
        subject: "not an exact host operation",
      });
      const unrelated = await request("patchbay-notify-host-operation", {
        sessionId,
        ...runIdentity,
        subject: "conduit/unrelated-host-operation",
      });
      const stale = await request("patchbay-pump-exact-run", {
        sessionId,
        ...runIdentity,
        sourceRevision: runIdentity.sourceRevision + 1,
        quantum: 1,
      });
      const snapshot = await request("patchbay-snapshot-exact-run", {
        sessionId,
        ...runIdentity,
      });
      const cancelled = await request("patchbay-cancel-exact-run", {
        sessionId,
        ...runIdentity,
        disposition: "abort",
      });
      const terminalEvidence = await request("patchbay-read-exact-evidence", {
        sessionId,
        ...runIdentity,
        cursor: 0,
        maximumEvents: 256,
      });
      const presentationAcknowledge = await request(
        "patchbay-acknowledge-exact-evidence",
        { sessionId, cursor: terminalEvidence.value?.next_cursor ?? 0 },
      );
      const viewed = await request("patchbay-session-view", { sessionId });
      const disposed = await request("patchbay-dispose-exact-run", {
        sessionId,
        ...runIdentity,
      });
      return {
        configured,
        opened,
        started,
        attachedWatch,
        pumped,
        watch,
        detachedWatch,
        evidence,
        repeatedEvidence,
        invalidCandidate,
        activeAfterCandidate,
        malformed,
        unrelated,
        stale,
        snapshot,
        cancelled,
        terminalEvidence,
        presentationAcknowledge,
        viewed,
        disposed,
      };
    } finally {
      worker.terminate();
    }
  });

  expect(result.configured).toMatchObject({ ok: true, value: { configured: true } });
  expect(result.opened.value).toMatchObject({ ok: true, session_id: "tour/worker-exact-session" });
  expect(result.started.value).toMatchObject({ ok: true, state: "active" });
  expect(result.started.value.view.plan.watch_admissions[0]).toMatchObject({
    retention: "latest",
    maximum_history: 1,
    sensitivity_ceiling: "public",
  });
  expect(result.attachedWatch.value).toMatchObject({
    ok: true,
    attached: true,
    plan_identity: result.started.value.plan_identity,
    source_semantic_hash: result.started.value.source_semantic_hash,
  });
  expect(result.pumped.value).toMatchObject({ ok: true, state: "active" });
  expect(result.watch.value).toMatchObject({
    ok: true,
    status: { kind: "available" },
    plan_identity: result.started.value.plan_identity,
    source_semantic_hash: result.started.value.source_semantic_hash,
    records: [{
      material: { kind: "preview", text: "hello\n" },
      truncated: false,
    }],
  });
  expect(result.detachedWatch.value).toMatchObject({
    ok: true,
    attached: false,
    usage: { attached_slots: 0, retained_observations: 1 },
  });
  expect(result.evidence.value).toMatchObject({
    ok: true,
    status: { kind: "available" },
  });
  expect(result.evidence.value.records).toHaveLength(1);
  expect(result.repeatedEvidence.value).toEqual(result.evidence.value);
  expect(result.invalidCandidate.value).toMatchObject({
    ok: true,
    result: { compatibility: { compatible: false, plan_disposition: "unavailable" } },
  });
  expect(result.activeAfterCandidate.value.view).toMatchObject({
    source: { revision: 1 },
    run: { state: "Active", source_semantic_hash: result.started.value.source_semantic_hash },
  });
  expect(result.malformed.value).toMatchObject({ ok: false, code: "CND-PBY-012" });
  expect(result.unrelated.value).toMatchObject({ ok: true, state: "active" });
  expect(result.stale.value).toMatchObject({ ok: false, code: "CND-PBY-016" });
  expect(result.snapshot.value).toMatchObject({
    ok: true,
    run_id: result.started.value.run_id,
    source_revision: result.started.value.source_revision,
    plan_identity: result.started.value.plan_identity,
  });
  expect(result.cancelled.value).toMatchObject({ ok: true, state: "cancelled" });
  expect(result.terminalEvidence.value).toMatchObject({
    ok: true,
    status: { kind: "available" },
  });
  expect(result.terminalEvidence.value.records.at(-1)).toMatchObject({
    event_kind: "terminal",
    terminal_cause: "cancelled",
  });
  expect(result.presentationAcknowledge).toMatchObject({
    ok: false,
    code: "unsupported-operation",
  });
  expect(result.viewed.value.view.run.state).toBe("Terminal");
  expect(result.disposed.value).toMatchObject({
    ok: true,
    disposed_run_id: result.started.value.run_id,
  });
  expect(result.disposed.value.view.run).toBeUndefined();
});

test("starts one public latest-value Watch with bounded accounting", async ({ page }) => {
  const failures = collectPageFailures(page);
  await openTinyInstrument(page);
  await expect(page.locator("#title")).toHaveText(
    "Project one: A living instrument — Wake the instrument",
  );
  await expect(page.locator("#source")).toHaveValue(
    /conduit\.media\/control\/clock-divider/,
  );
  await expect(page.locator("#source")).toHaveValue(
    /conduit\.media\/control\/sequencer/,
  );
  await expect(page.locator("#source")).toHaveValue(
    /conduit\.media\/control\/mixer/,
  );
  await expect(page.locator("#instrument-result")).toBeVisible();
  await expect(page.locator("#instrument-result-text")).toContainText(
    "Start the exact run to produce the first beat",
  );
  await page.locator("#run").click();
  await expect.poll(async () => {
    const text = await page.locator("#watch-value").textContent();
    return parseWatchTick(text);
  }, { timeout: 20_000 }).toBeGreaterThanOrEqual(0);
  const firstTick = parseWatchTick(
    await page.locator("#watch-value").textContent(),
  );
  await expect(page.locator("#instrument-result")).toHaveAttribute(
    "data-tick",
    String(firstTick),
  );
  await expect(page.locator("#instrument-result-text")).toContainText(
    `Exact Watch beat ${firstTick}`,
  );
  await expect(page.locator("#instrument-result-text")).toContainText(
    "Audio remains off",
  );
  await expect(page.locator("#console-status-badge")).toHaveText("Live");
  await expect(page.locator("#result")).toContainText(/run remains waiting/i);
  const first = await page.locator("#watch-accounting").evaluate((element) =>
    JSON.parse(element.textContent)
  );
  expect(first).toMatchObject({
    state: "waiting",
    retention: "latest",
    representation: { id: "std/text" },
    sensitivity: "public",
    value_storage: { resident_slots: 0, resident_bytes: 0 },
    evidence_store: {
      earliest_cursor: expect.any(Number),
      next_cursor: expect.any(Number),
      retained_events: expect.any(Number),
      retained_bytes: expect.any(Number),
      maximum_events: 256,
      maximum_bytes: 262144,
      dropped_events: expect.any(Number),
    },
  });

  expect(first.cursor).toBeGreaterThan(0);
  expect(first.next_timer_deadline).toEqual(expect.any(Number));
  expect(first.value_storage.resident_slots).toBeLessThanOrEqual(
    first.value_storage.maximum_slots,
  );
  expect(first.value_storage.resident_bytes).toBeLessThanOrEqual(
    first.value_storage.maximum_bytes,
  );
  expect(first.value_storage.high_water_slots).toBeLessThanOrEqual(
    first.value_storage.maximum_slots,
  );
  expect(first.value_storage.high_water_bytes).toBeLessThanOrEqual(
    first.value_storage.maximum_bytes,
  );
  expect(first.evidence_store.next_cursor).toBeGreaterThan(0);
  expect(first.evidence_store.retained_events).toBeLessThanOrEqual(
    first.evidence_store.maximum_events,
  );
  expect(first.evidence_store.retained_bytes).toBeLessThanOrEqual(
    first.evidence_store.maximum_bytes,
  );
  await expect(page.locator("#live-flow-status")).toContainText(
    "authoritative event",
  );
  await expect(page.locator("#watch-observation-lead")).toHaveAttribute(
    "data-attached",
    "true",
  );
  await expect(page.locator("#watch-observation-lead")).toContainText(
    "cannot carry demand or pressure",
  );
  expect(await page.locator("#live-flow-table tbody tr").count()).toBeGreaterThan(0);
  expect(await page.locator("#live-flow-table tbody tr").count())
    .toBeLessThanOrEqual(12);
  const liveEdge = page.locator('.react-flow__edge[data-live-update="true"]').first();
  await expect(liveEdge).toHaveAttribute("data-live-sequence", /\d+/);
  await expect(liveEdge).toHaveAttribute("data-occupancy-items", /\d+/);
  await expect(page.locator("#freeze-display")).toHaveAttribute(
    "aria-keyshortcuts",
    "F",
  );
  expect(failures).toEqual([]);
});

test("freezes and resumes a live Watch without pressuring execution", async ({ page }) => {
  const failures = collectPageFailures(page);
  await openTinyInstrument(page);
  await page.emulateMedia({ reducedMotion: "reduce" });
  const liveEdge = page.locator('.react-flow__edge[data-live-update="true"]').first();
  await page.locator("#run").click();
  await expect(page.locator("#freeze-display")).toBeEnabled({ timeout: 20_000 });
  await page.locator("#freeze-display").click();
  await expect(page.locator("#freeze-display")).toHaveAttribute("aria-pressed", "true");
  await expect.poll(async () => parseWatchTick(
    await page.locator("#watch-value").textContent(),
  ), { timeout: 20_000 }).toBeGreaterThanOrEqual(0);
  const beforeFreeze = parseWatchTick(
    await page.locator("#watch-value").textContent(),
  );
  await expect(page.locator("#display-freeze-status")).toContainText(
    "deferred while the exact executor remains live",
    { timeout: 20_000 },
  );
  expect(parseWatchTick(await page.locator("#watch-value").textContent()))
    .toBe(beforeFreeze);
  await page.locator("#freeze-display").click();
  await expect(page.locator("#freeze-display")).toHaveAttribute("aria-pressed", "false");
  await expect.poll(async () => parseWatchTick(
    await page.locator("#watch-value").textContent(),
  ), { timeout: 20_000 }).toBeGreaterThan(beforeFreeze);
  await expect(page.locator(".watch-semantics")).toContainText(
    "Watch is isolated instrumentation",
  );
  await expect(page.locator(".watch-semantics")).toContainText(
    "semantic tee changes this panel and its exact plan",
  );
  await expect(page.locator("#watch-toggle")).toHaveAttribute("aria-keyshortcuts", "W");
  await expect(page.locator("#watch-toggle")).toHaveAttribute("aria-pressed", "true");
  await expect(liveEdge).toHaveAttribute("data-live-sequence", /\d+/);
  await expect(liveEdge).not.toHaveClass(/live-flow-pulse/);
  expect(failures).toEqual([]);
});

test("detaches a live Watch without pressuring execution", async ({ page }) => {
  const failures = collectPageFailures(page);
  const { first } = await startTinyInstrument(page);
  const watchToggle = page.locator("#watch-toggle");
  const structuredCordWatch = page.locator(
    "#panel-connection-list .structured-watch-button",
  ).filter({ hasText: "Remove Watch" });
  await expect(structuredCordWatch).toHaveCount(1);
  await expect(structuredCordWatch).toContainText("Remove Watch");
  await expect(watchToggle).toBeEnabled({ timeout: 20_000 });
  await structuredCordWatch.click();
  await expect(watchToggle).toHaveAttribute("aria-pressed", "false", {
    timeout: 20_000,
  });
  await expect(watchToggle).toBeEnabled({ timeout: 20_000 });
  await expect(page.locator("#console-status-badge")).toHaveText("Live");
  await expect.poll(async () => {
    const accounting = await page.locator("#watch-accounting").evaluate((element) =>
      JSON.parse(element.textContent)
    );
    return accounting.attached ?? null;
  }, { timeout: 20_000 }).toBe(false);
  await expect.poll(async () => {
    const accounting = await page.locator("#watch-accounting").evaluate((element) =>
      JSON.parse(element.textContent)
    );
    return accounting.evidence_store.next_cursor;
  }, { timeout: 20_000 }).toBeGreaterThan(first.evidence_store.next_cursor);
  const detached = await page.locator("#watch-accounting").evaluate((element) =>
    JSON.parse(element.textContent)
  );
  expect(detached).toMatchObject({
    attached: false,
    state: "waiting",
    run_id: first.run_id,
    plan_identity: first.plan_identity,
    source_semantic_hash: first.source_semantic_hash,
  });
  await expect(page.locator("#watch-value")).toContainText(
    "the exact ticker continues without observation pressure",
  );
  await expect(page.locator("#watch-observation-lead")).toHaveAttribute(
    "data-attached",
    "false",
  );
  expect(failures).toEqual([]);
});

test("reattaches a live Watch without pressuring execution", async ({ page }) => {
  const failures = collectPageFailures(page);
  const { firstTick } = await startTinyInstrument(page);
  const watchToggle = page.locator("#watch-toggle");
  await expect(watchToggle).toBeEnabled({ timeout: 20_000 });
  await watchToggle.click();
  await expect(watchToggle).toHaveAttribute("aria-pressed", "false", {
    timeout: 20_000,
  });

  await watchToggle.click();
  await expect(watchToggle).toHaveAttribute("aria-pressed", "true", {
    timeout: 20_000,
  });
  await expect(watchToggle).toBeEnabled({ timeout: 20_000 });
  await expect.poll(async () => parseWatchTick(
    await page.locator("#watch-value").textContent(),
  ), { timeout: 20_000 }).toBeGreaterThan(firstTick);
  expect(failures).toEqual([]);
});

test("links an active Watch cord event to its exact source", async ({ page }) => {
  const failures = collectPageFailures(page);
  await startTinyInstrument(page);
  const liveCordEvent = page.locator(
    '.timeline-event[data-subject-kind="cord"]',
  ).last();
  await liveCordEvent.click();
  await expect(page.locator(".react-flow__edge.selection-current")).toHaveCount(1);
  expect(await page.locator("#source").evaluate((element) =>
    element.selectionEnd > element.selectionStart
  )).toBe(true);
  expect(failures).toEqual([]);
});

test("toggles an admitted Watch with W from non-editing focus", async ({ page }) => {
  const failures = collectPageFailures(page);
  await startTinyInstrument(page);
  const watchToggle = page.locator("#watch-toggle");
  await expect(watchToggle).toBeEnabled({ timeout: 20_000 });
  const freezeDisplay = page.locator("#freeze-display");
  await expect(freezeDisplay).toBeEnabled();
  await expect.poll(async () => {
    const pressed = await watchToggle.getAttribute("aria-pressed");
    if (pressed === "true") {
      await freezeDisplay.focus();
      await page.keyboard.press("w");
    }
    return watchToggle.getAttribute("aria-pressed");
  }, { timeout: 10_000 }).toBe("false");
  expect(failures).toEqual([]);
});

test("keeps the active Watch epoch exact across candidate edits and stop", async ({ page }) => {
  const failures = collectPageFailures(page);
  await page.emulateMedia({ reducedMotion: "reduce" });
  const { browserPlan, first } = await startTinyInstrument(page);
  const activeValueBeforeEdit = parseWatchTick(
    await page.locator("#watch-value").textContent(),
  );
  await page.locator("#source").evaluate((element) => {
    element.value = `${element.value}\ncord`;
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await expect(page.locator(".patchbay-live-run-status")).toContainText(
    "is separate from this active epoch",
  );
  await expect(page.locator("#workspace-error-count")).not.toHaveText("0");
  await expect.poll(async () => parseWatchTick(
    await page.locator("#watch-value").textContent(),
  ), { timeout: 20_000 }).toBeGreaterThan(activeValueBeforeEdit);
  const afterCandidateEdit = await page.locator("#watch-accounting").evaluate((element) =>
    JSON.parse(element.textContent)
  );
  expect(afterCandidateEdit.run_id).toBe(first.run_id);
  expect(afterCandidateEdit.plan_identity).toBe(first.plan_identity);
  expect(browserPlan.evidence_provider).toMatchObject({
    implementation_id: "conduit/browser-worker-exact-evidence",
    retention: "rolling",
    maximum_events: first.evidence_store.maximum_events,
    maximum_bytes: first.evidence_store.maximum_bytes,
    maximum_projection_events: 32,
    gap_policy: "explicit-earliest-cursor",
    terminal_required: true,
    storage_claim: "execution-plan-budget.evidence_bytes",
    provider_resource: null,
  });

  await page.locator("#stop").click();
  await expect(page.locator("#console-status-badge")).toHaveText("Ready");
  await expect(page.locator("#result")).toContainText(
    "Run cancelled; exact worker placement is terminal.",
  );
  await expect(page.locator("#run")).toBeEnabled();
  await expect(page.locator("#stop")).toBeDisabled();
  expect(failures).toEqual([]);
});

test("keeps live textual instrumentation truthful when the topology renderer is unavailable", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.addInitScript(() => {
    window.__CONDUIT_DISABLE_PATCHBAY_RENDERER__ = true;
  });
  await gotoTour(page, "/tour/public/index.html?lesson=panels.tiny-instrument");
  await expect(page.locator("#run")).toBeEnabled({ timeout: 20_000 });
  await page.locator("#run").click();
  await expect(page.locator("#cy")).toContainText("React Flow renderer unavailable.");
  await expect(page.locator("#live-flow-status")).toContainText(
    "authoritative event",
    { timeout: 20_000 },
  );
  await expect(page.locator("#live-flow-table tbody tr")).not.toHaveCount(0);
  await expect(page.locator("#console-status-badge")).toHaveText("Live");
  await page.locator("#stop").click();
});

test("presents the persistent HTTP source and refuses to simulate its hosted provider", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto(
    "/tour/public/index.html?lesson=library.bounded-http-service",
  );

  await expect(page.locator("#title")).toHaveText(
    "Project two: A small service that stays alive — Open the service boundary",
  );
  await expect(page.locator("#source")).toHaveValue(/server: net\/http\/listen/);
  await expect(page.locator("#source")).toHaveValue(/deadline_ticks = "5000"/);
  await expect(page.locator("#runnability-state")).toHaveText(
    "illustrative/unavailable · browser",
  );
  await expect(page.locator("#execution-note")).toContainText("CND-IMP-001");
  await expect(page.locator("#result")).toContainText("CND-IMP-001", {
    timeout: 20_000,
  });
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator('.react-flow__node[data-id="server"]')).toBeVisible();
  await expect(page.locator("#prose")).toContainText(
    "one authorized Start binds one loopback listener",
  );
  await expect(page.locator("#prose")).toContainText(
    "they never substitute fetch, a JavaScript server, or a replay animation",
  );
  await expect(
    page.locator('.react-flow__node[data-id="server"]'),
  ).toHaveCSS("animation-name", "none");
});

test("runs with Shift+Enter from editor and workspace focus", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
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
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await source.fill((await source.inputValue()).replace("Hello from the Tour.", "Recover me."));
  await page.locator("#reset").click();
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await page.locator("#undo-reset").click();
  await expect(source).toHaveValue(/Recover me\./);
});

test("highlights panel source while retaining the native editor surface", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
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
      "  > in: audio/pcm-stream\n" +
      "  in >: speech/transcript\n" +
      "  > audio: audio/pcm-stream\n" +
      "  committed >: speech/transcript\n" +
      "}\nvalue: fixture/source implements speech/recognizer\n",
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
      "  > in: audio/pcm-stream\n" +
      "  in >: speech/transcript\n" +
      "  > audio: audio/pcm-stream\n" +
      "  committed >: speech/transcript\n" +
      "}\nvalue: fixture/source implements speech/recognizer\n",
  );

  await source.fill(
    "panel 0\nexample/uppercase {\n" +
      "  worker: text/uppercase\n" +
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

  await source.fill(
    "panel 0\nκαφές: fixture/source\nadults: fixture/sink\n" +
      "καφές > keep { it > 18 } > adults\n",
  );
  await expect(highlight).toHaveAttribute("data-semantic-metadata", "available");
  await expect(highlight.locator(".panel-token-identifier").filter({
    hasText: /^καφές$/,
  })).toHaveCount(2);
  await expect(highlight.locator(".panel-token-operator-graph")).toHaveCount(2);
  await expect(highlight.locator(".panel-token-operator-expression")).toHaveCount(1);
  await expect(highlight.locator(".panel-token-operator-graph").first())
    .toHaveAttribute("data-token-label", "graph connect operator");
  await expect(highlight.locator(".panel-token-operator-expression"))
    .toHaveAttribute("data-token-label", "expression greater-than operator");

  await source.fill('panel 0\ninterface broken {\n  > audio: "not > metadata"\n');
  await expect(highlight).toHaveAttribute("data-semantic-metadata", "unavailable");
  await expect(highlight.locator(".panel-token-port")).toHaveCount(0);
  await expect(highlight.locator(".panel-token-port-sigil")).toHaveCount(0);
  await expect(source).toHaveValue(
    'panel 0\ninterface broken {\n  > audio: "not > metadata"\n',
  );
  await expect(highlight).toHaveAttribute("aria-hidden", "true");
});

test("covers every published chapter and exposes production topology projections", async ({
  page,
}) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const readerCatalog = await page.evaluate(async () => {
    const response = await fetch("../book/current.json", { cache: "no-store" });
    const catalog = await response.json();
    return {
      projects: catalog.projects.length,
      chapters: catalog.projects.flatMap((project) => project.chapters).length,
      sections: catalog.projects.flatMap((project) =>
        project.chapters.flatMap((chapter) => chapter.sections)).length,
    };
  });
  expect(readerCatalog).toEqual({ projects: 4, chapters: 6, sections: 20 });
  await expect(page.locator(".toc-project")).toHaveCount(readerCatalog.projects);
  await expect(page.locator(".toc-chapter")).toHaveCount(readerCatalog.chapters);
  await expect(page.locator("[data-section-id]")).toHaveCount(readerCatalog.sections);
  await gotoTour(page, "/tour/public/index.html?lesson=panels.inside-outside");
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
    '"instance": "root/box.worker"',
  );
  await expect(page.locator("#topology")).toContainText('"contract_id": "text/uppercase"');
  await expect(page.locator("#plan-view-notice")).toContainText(
    "read-only candidate plan",
  );
  const plannedWorker = page.locator('.react-flow__node[data-id="root/box.worker"]');
  await expect(plannedWorker.locator('[data-clue="implementation"]')).toHaveCount(1);
  await expect(plannedWorker.locator('[data-clue="provider"]')).toHaveCount(1);
  await expect(plannedWorker.locator('[data-clue="artifact"]')).toHaveCount(1);
  await plannedWorker.locator(".faceplate-header").click();
  await expect(page.locator("#selection-inspector")).toBeVisible();
  await expect(page.locator('[data-section="realization"]')).toContainText(
    "implementation",
  );
  await expect(page.locator('[data-section="realization"]')).toContainText(
    "provider observation",
  );
  await expect(page.locator('[data-section="realization"]')).toContainText(
    "artifact",
  );
  await expect(page.locator("#panel-port-list")).toContainText("root/box.worker: > text");
  await expect(page.locator("#panel-port-list")).toContainText("root/box.worker: text >");
  await page.locator("#logical-view").click();
  await expect(page.locator("#logical-view")).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#expanded-view")).toHaveAttribute("aria-pressed", "false");
  await expect(page.locator("#topology")).toContainText('"id": "box"');
  await expect(page.locator("#topology")).toContainText(
    '"contract_id": "example/upper-box"',
  );
  await expect(page.locator("#topology")).not.toContainText("worker");
});

test("accepts a semantically correct alternate solution", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/Hello from the Tour\./);
  await source.fill(
    (await source.inputValue())
      .replace("greeting:", "salutation:")
      .replace("greeting.value", "salutation.value"),
  );
  await page.locator("#run").click();
  await expect(page.locator("#result")).toContainText("✓ Lesson complete!", {
    timeout: 20_000,
  });
  await expect(source).toHaveValue(/salutation/);
});

test("keeps Expanded unavailable when the semantic revision has no exact plan", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  await source.fill("panel 0\nunfinished :");
  await expect(page.locator("#expanded-view")).toBeDisabled();
  await expect(page.locator("#logical-view")).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#plan-view-notice")).toContainText(
    "No exact plan has been resolved",
  );
  await expect(page.locator("#plan-view-notice")).toContainText(
    "no realization is manufactured from registry defaults",
  );
});

test("keeps structural lenses orthogonal to Use Build Inspect and preserves the exact subject", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const workspace = page.locator("#workspace");
  const status = page.locator("#presentation-status");
  await expect(workspace).toHaveAttribute("data-presentation-mode", "build");
  await expect(workspace).toHaveAttribute("data-structural-lens", "face");
  await expect(workspace).toHaveAttribute("data-topology-projection", "logical");
  await expect(status).toContainText(
    "No usable task front is declared, so this root opened in Build.",
  );

  await page.getByRole("button", {
    name: "value, outgoing port; type std/text",
    exact: true,
  }).click();
  await expect(status).toContainText("root/greeting/port/outgoing/value");

  const sourceBefore = await page.locator("#source").inputValue();
  await expect(page.locator('[data-presentation-mode="use"]')).toBeDisabled();
  await page.locator('[data-presentation-mode="inspect"]').click();
  await expect(workspace).toHaveAttribute("data-presentation-mode", "inspect");
  await expect(page.locator("#source")).toHaveAttribute("readonly", "");
  await expect(status).toContainText("root/greeting/port/outgoing/value");

  await page.locator("#show-how").click();
  await expect(workspace).toHaveAttribute("data-presentation-mode", "build");
  await expect(page.locator(".source-card")).toBeVisible();
  await expect(page.locator("#source")).toHaveValue(sourceBefore);
  await expect(status).toContainText("root/greeting/port/outgoing/value");

  await page.locator("#show-why").click();
  await expect(workspace).toHaveAttribute("data-presentation-mode", "inspect");
  await expect(workspace).toHaveAttribute("data-structural-lens", "context");
  await expect(page.locator("#source")).toHaveAttribute("readonly", "");
  await page.locator("#expanded-view").click();
  await expect(workspace).toHaveAttribute("data-topology-projection", "expanded");
  await expect(workspace).toHaveAttribute("data-presentation-mode", "inspect");
  await expect(workspace).toHaveAttribute("data-structural-lens", "context");

  await page.locator('[data-structural-lens="configure"]').click();
  await expect(page.locator("#configuration-layers")).toBeVisible();
  await expect(page.locator("#configuration-layer-list")).toContainText(
    "Owner: panel-instance",
  );
  await expect(page.locator("#configuration-layer-list")).toContainText(
    "Owner: exact-plan",
  );
  await expect(page.locator("#configuration-layer-list")).toContainText(
    "activation: re-resolution-or-plan-transition",
  );
});

test("keeps the Use information budget usable at two hundred percent zoom", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=panels.jacks-on-the-front");
  const workspace = page.locator("#workspace");
  const front = page.locator("#task-front");
  await expect(workspace).toHaveAttribute("data-presentation-mode", "use");
  await expect(front).toBeVisible();
  await expect(front).toHaveAttribute("data-descriptor-identity", /^sha256:/);
  await expect(page.locator(".source-card")).toBeHidden();
  await expect(page.locator(".inspectors")).toBeHidden();
  await expect(page.locator(".task-front-control")).toHaveCount(1);
  await expect(front).not.toContainText("private_worker");
  await expect(page.getByLabel("Uppercase text input")).toBeDisabled();
  await expect(front).toContainText("run-only");
  await expect(front).toContainText("current-run-delivery");
  await expect(page.getByRole("button", { name: "Run the checked uppercase-text plan" })).toBeEnabled();

  await page.evaluate(() => {
    document.documentElement.style.zoom = "200%";
  });
  const actionBox = await page.locator(".task-front-primary-action").boundingBox();
  const resultBox = await page.locator("#task-front-result").boundingBox();
  expect(actionBox).not.toBeNull();
  expect(resultBox).not.toBeNull();
  expect(actionBox.x).toBeLessThan(page.viewportSize().width);
  expect(resultBox.x).toBeLessThan(page.viewportSize().width);

  await page.locator("#show-how").click();
  await expect(workspace).toHaveAttribute("data-presentation-mode", "build");
  await expect(page.locator("#source")).toHaveValue(/private_worker/);
  await expect(front).toBeHidden();
  await page.locator('[data-presentation-mode="use"]').click();
  await expect(front).toBeVisible();
  await page.locator("#show-why").click();
  await expect(workspace).toHaveAttribute("data-presentation-mode", "inspect");
  await expect(page.locator("#presentation-status")).toContainText(
    "root/faceplate/port/outgoing/result",
  );
});

test("runs the task-front action without manufacturing a semantic result", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=panels.jacks-on-the-front");
  await page.getByRole("button", { name: "Run the checked uppercase-text plan" }).click();
  await expect(page.locator("#result")).toContainText("JACKS", { timeout: 20_000 });
  await expect(page.locator("#task-front-result-value")).toContainText(
    "terminal-without-semantic-result-observation",
  );
});

test("navigates composite boundaries from canvas and structured controls", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=panels.inside-outside");
  const workspace = page.locator("#workspace");
  const status = page.locator("#presentation-status");
  const sourceBefore = await page.locator("#source").inputValue();

  await page.getByRole("button", { name: "Open box inside" }).first().click();
  await expect(workspace).toHaveAttribute("data-structural-lens", "inside");
  await expect(status).toContainText("root/box");
  await expect(page.locator("#topology")).toContainText('"owner": "panel-definition"');
  await expect(page.locator("#source")).toHaveValue(sourceBefore);

  await page.locator("#panel-boundary-list").getByRole("button", {
    name: "Open box context",
  }).click();
  await expect(workspace).toHaveAttribute("data-structural-lens", "context");
  await expect(page.locator("#topology")).toContainText('"owner_kind": "enclosing-panel"');
  await expect(page.locator("#topology")).toContainText("realization_bindings");
  await expect(status).toContainText("root/box");

  const atRestButton = page.locator('[data-structural-lens="at-rest"]');
  await atRestButton.evaluate((button) =>
    button.scrollIntoView({ block: "center", behavior: "instant" })
  );
  await atRestButton.click();
  await expect(workspace).toHaveAttribute("data-structural-lens", "at-rest");
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator(".primary-actions")).toBeHidden();
  await expect(page.locator("#topology")).toContainText(
    '"provider_availability": "not-observed"',
  );
  await expect(page.locator("#topology")).toContainText('"resolved": false');
  await expect(page.locator("#topology")).toContainText('"run_started": false');
  const faceButton = page.locator('[data-structural-lens="face"]');
  await faceButton.evaluate((button) =>
    button.scrollIntoView({ block: "center", behavior: "instant" })
  );
  await faceButton.click();
  await expect(workspace).toHaveAttribute("data-structural-lens", "face");
  await expect(page.locator(".primary-actions")).toBeVisible();
  await expect(page.locator("#topology")).not.toContainText("worker");
  await expect(page.locator("#topology")).toContainText(
    '"owner": "panel-instance-public-boundary"',
  );
  await expect(page.locator("#source")).toHaveValue(sourceBefore);
});

test("uses React Flow with legacy line placement disabled", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const canvas = page.locator("#patchbay-flow-root");
  await expect(canvas).toHaveAttribute("data-renderer", "react-flow");
  await expect(canvas).toHaveAttribute("data-projection", "rust-authoritative");
  await expect(canvas).toHaveAttribute("data-legacy-line-placement", "false");
  await expect(canvas).toHaveAttribute("data-node-count", "2");
  await expect(canvas).toHaveAttribute("data-edge-count", "1");
  await expect(canvas).toHaveAttribute("data-run-state", "prepared");
  await expect(page.locator(".patchbay-live-run-status")).toHaveText(
    "No exact run started.",
  );
  await expect(page.locator(".conduit-faceplate-card")).toHaveCount(2, {
    timeout: 20_000,
  });
  const canvasBox = await canvas.boundingBox();
  const firstNodeBox = await page.locator(".react-flow__node").first().boundingBox();
  expect(canvasBox?.height).toBeGreaterThan(0);
  expect(firstNodeBox?.y).toBeGreaterThanOrEqual(canvasBox?.y ?? Infinity);
  expect(firstNodeBox?.y).toBeLessThan((canvasBox?.y ?? 0) + (canvasBox?.height ?? 0));
  await expect(page.locator(".semantic-promise-compartment")).toHaveCount(0);
  await expect(page.locator('[data-clue="kind"]')).toHaveCount(2);
  await expect(page.locator(".faceplate-status-label", { hasText: "provider" })).toHaveCount(0);
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
  await expect(page.locator(".faceplate-config-row")).toHaveCount(0);
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
  await expect(page.locator("#selection-inspector")).toBeVisible();
  await expect(page.locator('[data-section="port"]')).toContainText("std/text");
  await expect(page.locator(".panel-source-selection")).toHaveText("value");
  const selectedEndpoint = await page.locator("#source").evaluate((element) =>
    element.value.slice(element.selectionStart, element.selectionEnd)
  );
  expect(selectedEndpoint).toBe("value");

  const greetingBox = await page.locator('[data-id="greeting"] .conduit-faceplate-card')
    .boundingBox();
  expect(greetingBox?.height).toBeLessThan(260);
});

test("keeps compact topology details in one presentation-only selection inspector", async ({
  page,
}) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  const sourceBefore = await source.inputValue();
  const planBefore = await page.locator("#plan").textContent();
  const inspector = page.locator("#selection-inspector");

  await expect(inspector).toBeHidden();
  await page.locator('[data-id="greeting"] .faceplate-header').click();
  await expect(inspector).toBeVisible();
  await expect(page.locator("#selection-inspector-state")).toContainText(
    "instance · root/greeting",
  );
  await expect(page.locator('[data-section="semantic"]')).toContainText(
    "Semantic contract",
  );
  await expect(page.locator('[data-section="configuration"]')).toContainText(
    "Configuration",
  );
  await expect(source).toHaveValue(sourceBefore);
  await expect(page.locator("#plan")).toHaveText(planBefore);

  await page.locator("#selection-inspector-close").click();
  await expect(inspector).toBeHidden();
  await expect(page.locator("#selected-node-label")).toHaveText(
    "No topology item selected",
  );

  await page.locator('[data-id="greeting"] .faceplate-header').click();
  await page.keyboard.press("Escape");
  await expect(inspector).toBeHidden();
  await page.locator('[data-id="greeting"] .faceplate-header').click();
  await page.locator(".react-flow__pane").click({ position: { x: 8, y: 8 } });
  await expect(inspector).toBeHidden();

  await expect(page.locator("#console-body")).toBeHidden();
  await expect(page.locator("#console-disclosure")).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  await page.locator("#console-disclosure").click();
  await expect(page.locator("#console-body")).toBeVisible();
  await expect(source).toHaveValue(sourceBefore);
  await expect(page.locator("#plan")).toHaveText(planBefore);
});

test("keeps a representative many-node patch as compact selectable symbols", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  await gotoTour(page, "/tour/public/index.html?lesson=panels.tiny-instrument");
  const cards = page.locator(".conduit-faceplate-card");
  await expect.poll(() => cards.count(), { timeout: 20_000 }).toBeGreaterThan(10);
  const heights = await cards.evaluateAll((elements) =>
    elements.map((element) => element.getBoundingClientRect().height)
  );
  expect(Math.max(...heights)).toBeLessThan(340);
  await expect(page.locator(".semantic-promise-compartment")).toHaveCount(0);
  await expect(page.locator(".planned-realization-compartment")).toHaveCount(0);
  await expect(page.locator(".faceplate-config-row")).toHaveCount(0);

  const keyboardPort = page.locator(
    "#panel-port-list .structured-topology-button",
  ).first();
  await keyboardPort.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#selection-inspector")).toBeVisible();
  await page.setViewportSize({ width: 720, height: 720 });
  const inspectorBox = await page.locator("#selection-inspector").boundingBox();
  expect(inspectorBox?.x).toBeGreaterThanOrEqual(0);
  expect((inspectorBox?.x || 0) + (inspectorBox?.width || 0)).toBeLessThanOrEqual(720);
  expect(await page.evaluate(() => document.documentElement.scrollWidth))
    .toBeLessThanOrEqual(720);
});

test("draws bounded cords and exposes draggable rewire ends", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/greeting/, { timeout: 20_000 });
  await expect(page.locator("#patchbay-flow-root")).toHaveAttribute(
    "data-layout",
    "ready",
  );
  await source.fill(
    "panel 0\n\n" +
    "first: std/literal { value = \"first\" }\n" +
    "primary: display/text\n",
  );
  await expect(page.locator(".react-flow__node")).toHaveCount(2);

  const dragHandle = async (from, to) => {
    await expect(from).toBeVisible();
    await expect(to).toBeVisible();
    await from.scrollIntoViewIfNeeded();
    await to.scrollIntoViewIfNeeded();
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
  await expect(source).toHaveValue(/first\.value > primary\.text/);
  await expect(source).toHaveValue(/max_queued_bytes = 1024/);

  await clickCordPath(page, page.locator(".react-flow__edge").first());
  const updaters = page.locator(
    ".react-flow__edge.selection-current .react-flow__edgeupdater",
  );
  await expect(updaters).toHaveCount(2);
  await expect(updaters.first()).toHaveCSS("pointer-events", "all");
});

test("renders composite exports as public faceplate ports", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=panels.inside-outside");
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
  await expect(page.locator('.react-flow__node[data-id="root/box.worker"]')).toHaveCount(1);
  await page.locator(".react-flow__edge").first()
    .locator(".react-flow__edge-path").dispatchEvent("click");
  await expect(page.locator(".faceplate-port-row.selected-cord-endpoint")).toHaveCount(2);
});

test("keeps semantic port direction redundant across presentation media", async ({ page }) => {
  await page.emulateMedia({
    colorScheme: "light",
    forcedColors: "active",
    reducedMotion: "reduce",
  });
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
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
  await expect(page.locator(".patchbay-cord").first()).toHaveCSS(
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

test("keeps inspector controls focused while highlighting and updating source", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  await page.locator('[data-id="greeting"] .faceplate-header').click();
  const input = page.locator(
    '#selection-inspector .selection-inspector-control',
  );
  const selectedSourceText = async () =>
    (await page.locator(".panel-source-selection").allTextContents()).join("");
  await input.click();

  await expect(input).toBeFocused();
  await expect.poll(selectedSourceText).toContain("greeting");

  await input.fill("Edited on the faceplate.");
  await expect(input).toBeFocused();
  await expect(input).toHaveValue("Edited on the faceplate.");
  await expect(page.locator("#source")).toHaveValue(/Edited on the faceplate\./);
  await expect.poll(selectedSourceText).toContain("greeting");
});

test("selects a cord by authoritative identity and reveals its declaration", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const edge = page.locator(".react-flow__edge").first();
  await clickCordPath(page, edge);

  await expect(page.locator("#selected-node-label")).toContainText(
    "Selected cord: cord-0",
  );
  await expect(edge).toHaveClass(/selection-current/);
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
    page.locator(".panel-source-selection .panel-token-operator-graph").filter({
      hasText: /^>$/,
    }),
  ).toHaveCount(1);
  expect(highlighted).toContain("greeting.value > output.text");
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
    "Selected semantic node: greeting",
  );
  await expect.poll(async () =>
    (await page.locator(".panel-source-selection").allTextContents()).join("")
  ).toContain("greeting");
});

test("shows node movement while a topology box is being dragged", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
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

test("retains committed topology positions across Check and Run renders", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const greeting = page.locator('[data-id="greeting"]');
  const output = page.locator('[data-id="output"]');
  const committedTransform = await dragAndCommitTopologyNode(page, greeting, 96, 48);
  const committedOutputTransform = await dragAndCommitTopologyNode(page, output, -72, 40);

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
});

test("restores a committed topology position across lesson visits", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const greeting = page.locator('[data-id="greeting"]');
  const committedTransform = await dragAndCommitTopologyNode(page, greeting, 96, 48);
  await gotoTour(page, "/tour/public/index.html?lesson=panels.inside-outside");
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  await expect.poll(
    async () => greeting.evaluate((element) => element.style.transform),
  ).toBe(committedTransform);
});

test("restores a committed topology position across reload", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const greeting = page.locator('[data-id="greeting"]');
  const committedTransform = await dragAndCommitTopologyNode(page, greeting, 96, 48);
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute(
    "data-tour-ready",
    "true",
    { timeout: 20_000 },
  );
  await expect.poll(
    async () => greeting.evaluate((element) => element.style.transform),
  ).toBe(committedTransform);
});

test("retains headless editing and execution when presentation fails", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
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

test("clears the previous diagram before redrawing a lesson that fails resolution", async ({
  page,
}) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  await expect(page.locator('.react-flow__node[data-id="greeting"]')).toHaveCount(1);

  await page.evaluate(() => {
    history.pushState({}, "", "?lesson=nodes.types-mean-promises");
    dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(page.locator("#result")).toContainText("CND-IMP-001");
  await expect(page.locator('.react-flow__node[data-id="greeting"]')).toHaveCount(0);
  await expect(page.locator('.react-flow__node[data-id="source"]')).toHaveCount(1);
  await expect(page.locator('.react-flow__node[data-id="adapter"]')).toHaveCount(1);
  await expect(page.locator('.react-flow__node[data-id="sink"]')).toHaveCount(1);
  await expect(page.locator("#patchbay-flow-root")).toHaveAttribute(
    "data-projection",
    "rust-authoritative",
  );

  await page.evaluate(() => {
    history.pushState({}, "", "?lesson=nodes.empty-is-not-never");
    dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(page.locator('.react-flow__node[data-id="server"]')).toHaveCount(1);
  await expect(page.locator('.react-flow__node[data-id="empty"]')).toHaveCount(0);
  await expect(page.locator("#reader-section-title")).toHaveText(
    "Waiting is not completion",
  );
  await expect(page.locator("#patchbay-flow-root")).toHaveAttribute(
    "data-projection",
    "rust-authoritative",
  );
});

test("styles cords from their projected type and pressure policy", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const edge = page.locator(".patchbay-cord").first();
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

test("routes stacked reverse cords with straight rectilinear segments", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem(
      "conduit-tour-layout/welcome.hello-panel",
      JSON.stringify({
        greeting: { x: 300, y: 520 },
        output: { x: 320, y: 20 },
      }),
    );
  });
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");

  const path = page.locator(".patchbay-cord .react-flow__edge-path").first();
  await expect(path).toHaveAttribute("data-cord-geometry", "straight");
  await expect(path).toHaveAttribute("data-routing-mode", "rectilinear");
  const commands = await path.getAttribute("d");
  expect(commands).toMatch(/^M\s/);
  expect(commands).toMatch(/\bL\b/);
  expect(commands).not.toMatch(/\b[CQAS]\b/i);
});

test("renders the direction lesson as an invalid authored graph", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=nodes.direction-matters");

  await expect(page.locator('.react-flow__node[data-id="first"]')).toBeVisible();
  await expect(page.locator('.react-flow__node[data-id="second"]')).toBeVisible();
  await expect(
    page.locator('[data-id="second"] [data-port-direction="outgoing"]'),
  ).toContainText("value >");
  const edge = page.locator(".patchbay-cord");
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
  await page.getByRole("button", { name: "Show source diagnostics" }).click();
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
    "first.value > second.value",
  );
  await expect(page.locator("#plan")).toContainText(
    "No Rust-resolved plan for this source yet.",
  );
  await expect(page.locator("#run")).toBeDisabled();
  await expect(page.locator("#evidence")).not.toContainText('"event_kind"');
});

test("keeps current diagnostic source ranges marked as the source changes", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=nodes.direction-matters");
  const source = page.locator("#source");
  const diagnosticMark = page.locator(".panel-source-diagnostic");

  await expect(diagnosticMark).toHaveText("second.value");
  const original = await source.inputValue();
  const moved = `# moved diagnostic\n${original}`;
  await source.fill(moved);
  await expect(diagnosticMark).toHaveText("second.value");
  await expect.poll(async () => page.locator(".panel-source-highlight").evaluate(
    (highlight) => {
      const mark = highlight.querySelector(".panel-source-diagnostic");
      if (!mark) return -1;
      const prefix = document.createRange();
      prefix.setStart(highlight, 0);
      prefix.setEndBefore(mark);
      return prefix.toString().length;
    },
  )).toBe(moved.lastIndexOf("second.value"));

  const corrected = moved
    .replace(
      'second: std/literal {\n    value = "Second.\\n"\n}',
      "second: display/text",
    )
    .replace("first.value > second.value", "first.value > second.text");
  await source.fill(corrected);
  await expect(page.locator(".patchbay-cord")).toHaveClass(/cord-validity-valid/);
  await expect(diagnosticMark).toHaveCount(0);
});

test("keeps invalid, unresolved, incomplete, and corrected revisions distinct", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/greeting/, { timeout: 20_000 });
  const original = await source.inputValue();
  const destinationInvalid = "panel 0\n" +
    "greeting: std/literal { value = \"invalid\" }\n" +
    "output: display/text\n" +
    "greeting.value > greeting.value\n";
  await source.fill(destinationInvalid);
  await expect(source).toHaveValue(/greeting\.value > greeting\.value/);
  await expect(page.locator(".patchbay-cord")).toHaveClass(
    /cord-validity-wrong-direction/,
    { timeout: 20_000 },
  );
  await expect(page.locator("#run")).toBeDisabled();

  const incomplete = `${original}\nprovisional :`;
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

  await source.fill(`${original}\nprovisional: missing/contract\n`);
  await expect(
    page.locator('[data-id="provisional"] .conduit-faceplate-card'),
  ).toHaveClass(/faceplate-validity-unresolved/, { timeout: 20_000 });
  await expect(page.locator("#diagnostic-console")).toContainText(
    "No ports, provider, placement, or plan are inferred",
  );

  await source.fill(original);
  await expect(page.locator(".patchbay-cord")).toHaveClass(
    /cord-validity-valid/,
    { timeout: 20_000 },
  );
  await expect(page.locator('[data-id="greeting"]')).toBeVisible();
  await expect(page.locator("#run")).toBeEnabled();
});

test("projects every authored cord failure family with static reduced-motion cues", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/greeting/, { timeout: 20_000 });
  const cases = [
    {
      state: "wrong-direction",
      panel: "panel 0\na: display/text\nb: display/text\na.text > b.text\n",
    },
    {
      state: "unresolved",
      panel: "panel 0\nb: display/text\nmissing.value > b.text\n",
    },
    {
      state: "incompatible",
      panel: "panel 0\na: std/literal\nb: io/stdout\na.value > b.bytes\n",
    },
    {
      state: "invalid-bounds",
      panel: "panel 0\na: std/literal\nb: display/text\n" +
        "a.value > b.text { capacity = 1 max_value_bytes = 8 " +
        "max_queued_bytes = 8 low_watermark = 0 high_watermark = 2 pressure = block }\n",
    },
  ];
  for (const fixture of cases) {
    await source.fill(fixture.panel);
    await expect(source).toHaveValue(fixture.panel);
    const edge = page.locator(".patchbay-cord");
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
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/greeting/, { timeout: 20_000 });
  await source.fill(
    "panel 0\n" +
    "a: std/literal\n" +
    "b: std/literal\n" +
    "c: std/literal\n" +
    "a.value > b.value\n" +
    "b.value > c.value\n",
  );
  const edges = page.locator(".patchbay-cord");
  await expect(edges).toHaveCount(2);
  await expect(page.locator(".panel-source-diagnostic")).toHaveCount(2);
  await expect(edges.filter({ has: page.locator(".react-flow__edge-path") })).toHaveCount(2);
  await expect(page.locator(".patchbay-cord.diagnostic-emphasized")).toHaveCount(1);
  await expect(
    page.locator(".patchbay-cord:not(.diagnostic-emphasized)")
      .locator(".react-flow__edge-path"),
  ).toHaveCSS("animation-name", "none");

  const emphasizedPath = page.locator(
    ".patchbay-cord.diagnostic-emphasized .react-flow__edge-path",
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

test("enters and exits the same fullscreen workspace without rebuilding state", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/greeting/, { timeout: 20_000 });
  await page.locator("#expanded-view").click();
  await page.locator('.react-flow__node[data-id="root/greeting"]').click();
  await source.evaluate((element) => {
    element.__conduitLiveEditor = true;
    element.setSelectionRange(6, 14);
    element.scrollTop = 18;
  });
  const before = await page.evaluate(() => ({
    sourceRevision: JSON.parse(
      document.querySelector("#patchbay-editor-status").textContent
        .match(/r(\d+)/)[1],
    ),
    viewport: document.querySelector(".react-flow__viewport")?.style.transform,
    selection: [
      document.querySelector("#source").selectionStart,
      document.querySelector("#source").selectionEnd,
    ],
    expanded: document.querySelector("#expanded-view").getAttribute("aria-pressed"),
  }));

  await page.locator("#workspace-fullscreen").click();
  const workspace = page.locator("#patchbay-workspace");
  const fullscreenToggle = page.locator("#workspace-fullscreen");
  await expect(workspace).toHaveClass(/patchbay-workspace-active/);
  await expect.poll(() => page.evaluate(() =>
    document.fullscreenElement?.id || null
  )).toBe("patchbay-workspace");
  await expect(fullscreenToggle).toBeFocused();
  await expect(fullscreenToggle).toHaveAttribute("aria-pressed", "true");
  await expect(fullscreenToggle).toHaveAttribute(
    "aria-label",
    "Exit fullscreen Patchbay workspace",
  );
  await expect(page.locator("#workspace-exit")).toHaveCount(0);
  await expect(page.locator("#run")).toBeVisible();
  await expect(page.locator("#check")).toBeVisible();
  await expect(page.locator("#logical-view")).toBeVisible();
  await expect(page.locator("#arrange")).toBeVisible();
  await expect(page.locator("#workspace-error-count")).toBeVisible();
  await expect(page.locator("#patchbay-source-window")).toBeVisible();
  expect(await source.evaluate((element) => element.__conduitLiveEditor)).toBe(true);

  await fullscreenToggle.click();
  await expect(workspace).not.toHaveClass(/patchbay-workspace-active/);
  await expect(fullscreenToggle).toBeFocused();
  await expect(fullscreenToggle).toHaveAttribute("aria-pressed", "false");
  await expect(fullscreenToggle).toHaveAttribute(
    "aria-label",
    "Enter fullscreen Patchbay workspace",
  );
  await expect.poll(() => page.locator(".react-flow__viewport").evaluate(
    (element) => element.style.transform,
  )).toBe(before.viewport);
  const after = await page.evaluate(() => ({
    sourceRevision: JSON.parse(
      document.querySelector("#patchbay-editor-status").textContent
        .match(/r(\d+)/)[1],
    ),
    viewport: document.querySelector(".react-flow__viewport")?.style.transform,
    selection: [
      document.querySelector("#source").selectionStart,
      document.querySelector("#source").selectionEnd,
    ],
    expanded: document.querySelector("#expanded-view").getAttribute("aria-pressed"),
  }));
  expect(after).toEqual(before);
  await expect(page.locator("#selected-node-label")).toContainText(
    "Selected planned instance: root/greeting; source origin: greeting",
  );
  await page.locator("#workspace-fullscreen").click();
  await expect(workspace).toHaveClass(/patchbay-workspace-active/);
  await page.evaluate(() => document.exitFullscreen());
  await expect(workspace).not.toHaveClass(/patchbay-workspace-active/);
});

test("falls back honestly and keeps one movable shadeable dockable editor", async ({ page }) => {
  await page.addInitScript(() => {
    Element.prototype.requestFullscreen = () =>
      Promise.reject(new DOMException("denied", "NotAllowedError"));
  });
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/greeting/, { timeout: 20_000 });
  await source.evaluate((element) => {
    element.__conduitLiveEditor = "one";
    element.focus();
    element.setSelectionRange(8, 16);
    element.scrollTop = 24;
  });
  await page.locator("#workspace-fullscreen").click();
  const workspace = page.locator("#patchbay-workspace");
  const editorWindow = page.locator("#patchbay-source-window");
  await expect(workspace).toHaveClass(/patchbay-workspace-fallback/);
  await expect(page.locator("#workspace-mode-status")).toHaveText(
    "In-page fullscreen fallback",
  );
  await editorWindow.evaluate((element) =>
    Promise.all(
      element.getAnimations({ subtree: true })
        .map((animation) => animation.finished.catch(() => undefined)),
    )
  );

  const titlebar = page.locator(".patchbay-source-titlebar");
  const beforeDrag = await editorWindow.boundingBox();
  const titlebarBox = await titlebar.boundingBox();
  const titleBox = await titlebar.locator("h3").boundingBox();
  await page.mouse.move(
    titleBox.x + titleBox.width / 2,
    titleBox.y + titleBox.height / 2,
  );
  await page.mouse.down();
  await expect(editorWindow).toHaveClass(/workspace-dragging/);
  await expect(editorWindow).toHaveCSS("transition-duration", "0s");
  await page.mouse.move(titlebarBox.x + 125, titlebarBox.y + 75);
  await page.mouse.up();
  await expect.poll(async () => (await editorWindow.boundingBox()).x)
    .toBeGreaterThan(beforeDrag.x + 40);
  await expect.poll(async () => (await editorWindow.boundingBox()).y)
    .toBeGreaterThan(beforeDrag.y + 20);
  const afterDrag = await editorWindow.boundingBox();
  expect(afterDrag.x).toBeGreaterThan(beforeDrag.x + 40);
  expect(afterDrag.y).toBeGreaterThan(beforeDrag.y + 20);

  const resize = page.locator(".patchbay-editor-resize-handle");
  const beforeResize = await editorWindow.boundingBox();
  const resizeBox = await resize.boundingBox();
  await page.mouse.move(resizeBox.x + resizeBox.width / 2, resizeBox.y + resizeBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(resizeBox.x + 70, resizeBox.y + 55);
  await page.mouse.up();
  const afterResize = await editorWindow.boundingBox();
  expect(afterResize.width).toBeGreaterThan(beforeResize.width + 30);
  expect(afterResize.height).toBeGreaterThan(beforeResize.height + 20);

  const selection = await source.evaluate((element) => [
    element.selectionStart,
    element.selectionEnd,
    element.scrollTop,
  ]);
  await page.locator("#workspace-shade-editor").click();
  await expect(editorWindow).toHaveClass(/workspace-shaded/);
  await expect(page.locator("#patchbay-editor-status")).toContainText("diagnostic");
  await page.locator("#workspace-shade-editor").click();
  await expect(editorWindow).not.toHaveClass(/workspace-shaded/);
  await expect(source).toBeFocused();
  expect(await source.evaluate((element) => [
    element.selectionStart,
    element.selectionEnd,
    element.scrollTop,
  ])).toEqual(selection);
  expect(await source.evaluate((element) => element.__conduitLiveEditor)).toBe("one");

  await page.locator("#workspace-dock-editor").click();
  await expect(editorWindow).toHaveClass(/workspace-docked/);
  await expect(page.locator("#workspace-dock-editor")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.locator("#workspace-dock-editor").click();
  await expect(editorWindow).toHaveClass(/workspace-floating/);
  await page.locator("#workspace-hide-editor").click();
  await expect(editorWindow).toBeHidden();
  await expect(page.locator("#workspace-show-editor")).toBeVisible();
  await page.locator("#workspace-show-editor").click();
  await expect(editorWindow).toBeVisible();

  await page.setViewportSize({ width: 720, height: 540 });
  await expect.poll(async () => {
    const box = await editorWindow.boundingBox();
    return box.y + box.height;
  }).toBeLessThanOrEqual(540);
  const recovered = await editorWindow.boundingBox();
  expect(recovered.x).toBeGreaterThanOrEqual(0);
  expect(recovered.y).toBeGreaterThanOrEqual(0);
  expect(recovered.x + recovered.width).toBeLessThanOrEqual(720);
  expect(recovered.y + recovered.height).toBeLessThanOrEqual(540);

  await page.locator("#check").click();
  await expect(workspace).toHaveClass(/patchbay-workspace-active/);
  await page.locator("#workspace-shade-editor").click();
  await page.keyboard.press("Escape");
  await expect(workspace).not.toHaveClass(/patchbay-workspace-active/);
  await page.locator("#workspace-fullscreen").click();
  await expect(editorWindow).toHaveClass(/workspace-shaded/);
  await page.locator("#workspace-shade-editor").click();
  await page.keyboard.press("Escape");
  await expect(workspace).not.toHaveClass(/patchbay-workspace-active/);
});

test("navigates incomplete source and diagnostics in fullscreen with reduced motion", async ({ page }) => {
  await page.addInitScript(() => {
    Element.prototype.requestFullscreen = () =>
      Promise.reject(new DOMException("denied", "NotAllowedError"));
  });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await gotoTour(page, "/tour/public/index.html?lesson=nodes.direction-matters");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/first:/, { timeout: 20_000 });
  const original = await source.inputValue();
  await source.fill(`${original}\nprovisional :`);
  await expect(
    page.locator('[data-id="provisional"] .conduit-faceplate-card'),
  ).toHaveClass(/faceplate-validity-incomplete/, { timeout: 20_000 });
  await page.locator("#workspace-fullscreen").click();
  const workspace = page.locator("#patchbay-workspace");
  await expect(workspace).toHaveClass(/patchbay-workspace-active/);
  await expect(page.locator("#workspace-error-count")).not.toHaveAttribute(
    "data-count",
    "0",
  );
  await expect(page.locator("#patchbay-editor-status")).toContainText("edited");
  await page.locator("#workspace-error-count").click();
  await expect(page.locator(".patchbay-workspace-console")).toBeVisible();
  await page.locator(".diagnostic-console-button").first().click();
  await expect(source).toBeVisible();
  expect(await source.evaluate((element) =>
    element.selectionEnd > element.selectionStart
  )).toBe(true);
  await page.locator("#workspace-hide-editor").click();
  await page.locator("#workspace-hide-console").click();
  await page.locator(
    '[data-id="first"] .conduit-faceplate-card',
  ).click();
  await page.locator("#workspace-show-editor").click();
  await expect(page.locator(".panel-source-selection")).toContainText(
    "first",
  );
  await expect(workspace).toHaveCSS("animation-name", "none");
  await expect(page.locator("#patchbay-source-window")).toHaveCSS(
    "transition-duration",
    "0s",
  );
  await page.locator("#workspace-shade-editor").click();
  await page.locator("#workspace-shade-editor").click();
  await expect(
    page.locator(".patchbay-cord.diagnostic-emphasized .react-flow__edge-path"),
  ).toHaveCSS("animation-name", "none");
});

test("window presentation changes do not recreate unchanged diagnostic motion", async ({ page }) => {
  await page.addInitScript(() => {
    Element.prototype.requestFullscreen = () =>
      Promise.reject(new DOMException("denied", "NotAllowedError"));
  });
  await gotoTour(page, "/tour/public/index.html?lesson=nodes.direction-matters");
  await expect(page.locator(".patchbay-cord.diagnostic-emphasized")).toHaveCount(1);
  await page.locator("#workspace-fullscreen").click();
  const path = page.locator(
    ".patchbay-cord.diagnostic-emphasized .react-flow__edge-path",
  );
  await expect(path).toHaveCSS("animation-name", "patchbay-new-error");
  const before = await path.evaluate((element) => {
    element.__workspaceAnimation = element.getAnimations()[0];
    return element.__workspaceAnimation.currentTime;
  });
  await page.locator("#workspace-shade-editor").click();
  await page.locator("#workspace-dock-editor").click();
  await page.locator("#workspace-shade-editor").click();
  const after = await path.evaluate((element) => ({
    same: element.__workspaceAnimation === element.getAnimations()[0],
    currentTime: element.getAnimations()[0].currentTime,
  }));
  expect(after.same).toBe(true);
  expect(after.currentTime).toBeGreaterThan(before);
});

test("standalone Patchbay app exposes the same live fullscreen editor workspace", async ({ page }) => {
  await page.addInitScript(() => {
    Element.prototype.requestFullscreen = () =>
      Promise.reject(new DOMException("denied", "NotAllowedError"));
  });
  await page.goto("/tour/public/patchbay-app.html?lesson=welcome.hello-panel");
  const source = page.locator("#source");
  await expect(source).toHaveValue(/greeting/, { timeout: 20_000 });
  await source.evaluate((element) => {
    element.__standaloneEditorIdentity = "live";
  });
  await page.locator("#workspace-fullscreen").click();
  await expect(page.locator("#patchbay-workspace")).toHaveClass(
    /patchbay-workspace-active/,
  );
  await expect(page.locator("#patchbay-source-window")).toBeVisible();
  expect(
    await source.evaluate((element) => element.__standaloneEditorIdentity),
  ).toBe("live");
  await page.locator("#workspace-shade-editor").click();
  await expect(page.locator("#patchbay-source-window")).toHaveClass(
    /workspace-shaded/,
  );
});

test("Tour and standalone Patchbay consume the same checked task-front model", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=panels.jacks-on-the-front");
  const tourIdentity = await page.locator("#task-front").getAttribute(
    "data-descriptor-identity",
  );
  const tourSourceIdentity = await page.locator("#task-front").getAttribute(
    "data-source-identity",
  );
  await page.goto("/tour/public/patchbay-app.html?lesson=panels.jacks-on-the-front");
  const standalone = page.locator("#task-front");
  await expect(standalone).toBeVisible({ timeout: 20_000 });
  await expect(standalone).toHaveAttribute("data-descriptor-identity", tourIdentity);
  await expect(standalone).toHaveAttribute("data-source-identity", tourSourceIdentity);
  await expect(standalone.locator(".task-front-control")).toHaveCount(1);
});

test("routes cords through free space by default and keeps labels off node faces", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");

  const panelSource = "panel 0\n\n" +
    "source: std/literal {\n" +
    "  value = \"source\"\n" +
    "}\n" +
    "transform: text/uppercase\n" +
    "sink: display/text\n\n" +
    "source.value > transform.text {\n" +
    "  capacity = 1\n" +
    "  max_value_bytes = 1024\n" +
    "  max_queued_bytes = 1024\n" +
    "  low_watermark = 0\n" +
    "  high_watermark = 1\n" +
    "  pressure = block\n" +
    "}\n\n" +
    "transform.text > sink.text {\n" +
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

  const dragNodeTo = async (nodeId, relativeX, relativeY) => {
    const node = page.locator(`.react-flow__node[data-id="${nodeId}"]`);
    await expect(node).toHaveCount(1);
    await node.scrollIntoViewIfNeeded();
    const nodeBox = await node.boundingBox();
    const currentFlowBox = await flow.boundingBox();
    expect(nodeBox).not.toBeNull();
    expect(currentFlowBox).not.toBeNull();
    await page.mouse.move(nodeBox.x + nodeBox.width / 2, nodeBox.y + 20);
    await page.mouse.down();
    await page.mouse.move(
      currentFlowBox.x + relativeX,
      currentFlowBox.y + relativeY,
      { steps: 8 },
    );
    await page.mouse.up();
  };

  await dragNodeTo(
    "source",
    flowBox.width * 0.22,
    320,
  );
  await dragNodeTo(
    "transform",
    flowBox.width / 2,
    50,
  );
  await dragNodeTo(
    "sink",
    flowBox.width * 0.78,
    320,
  );

  const edge = page.locator(".patchbay-cord").nth(1);
  await expect(edge).toHaveCount(1);
  await expect.poll(async () =>
    edge.locator(".react-flow__edge-path").getAttribute("d")
  ).toMatch(/\bL\b/);
  await expect.poll(async () =>
    edge.locator(".react-flow__edge-path").getAttribute("d")
  ).not.toMatch(/\b[CQAS]\b/i);
  await expect
    .poll(async () => edge.locator(".react-flow__edge-path").getAttribute("d"))
    .not.toBe("");

  await expect.poll(async () => edge.evaluate((edgeElement, clearance) => {
    const path = edgeElement.querySelector(".react-flow__edge-path");
    if (!path) return false;
    const totalLength = path.getTotalLength();
    if (!Number.isFinite(totalLength) || totalLength <= 0) return false;
    const sampleCount = 240;
    const nodes = Array.from(document.querySelectorAll(".react-flow__node"))
      .map((node) => {
        const bounds = node.getBoundingClientRect();
        return {
          id: node.dataset.id,
          left: bounds.left - clearance,
          right: bounds.right + clearance,
          top: bounds.top - clearance,
          bottom: bounds.bottom + clearance,
        };
      });
    const endpointNodeIds = new Set([
      path.dataset.sourceNode,
      path.dataset.targetNode,
    ]);
    for (let index = 0; index <= sampleCount; index += 1) {
      const ratio = index / sampleCount;
      // A cord necessarily leaves its source faceplate and enters its target
      // faceplate. Its routed interior must remain in free space around every
      // other node, independent of route length or viewport scale.
      if (ratio < 0.03 || ratio > 0.97) continue;
      const point = path.getPointAtLength(totalLength * ratio);
      const screenPoint = point.matrixTransform(path.getScreenCTM());
      const hits = nodes.some(({ id, ...bounds }) =>
        !endpointNodeIds.has(id) &&
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
  }, 12)).toBe(false);

  await expect.poll(async () => edge.evaluate((edgeElement, clearance) => {
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
  }, 6)).toBe(false);

  await dragNodeTo("transform", 80, 70);
  await expect.poll(async () => {
    return edge.evaluate((edgeElement, clearance) => {
      const path = edgeElement.querySelector(".react-flow__edge-path");
      if (!path) return false;
      const totalLength = path.getTotalLength();
      if (!Number.isFinite(totalLength) || totalLength <= 0) return false;
      const sampleCount = 280;
      const nodes = Array.from(document.querySelectorAll(".react-flow__node"))
        .map((node) => {
          const bounds = node.getBoundingClientRect();
          return {
            id: node.dataset.id,
            left: bounds.left - clearance,
            right: bounds.right + clearance,
            top: bounds.top - clearance,
            bottom: bounds.bottom + clearance,
          };
        });
      const endpointNodeIds = new Set([
        path.dataset.sourceNode,
        path.dataset.targetNode,
      ]);
      for (let index = 0; index <= sampleCount; index += 1) {
        const ratio = index / sampleCount;
        if (ratio < 0.03 || ratio > 0.97) continue;
        const point = path.getPointAtLength(totalLength * ratio);
        const screenPoint = point.matrixTransform(path.getScreenCTM());
        const hits = nodes.some(({ id, ...bounds }) =>
          !endpointNodeIds.has(id) &&
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
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.hello-panel");
  await page.locator("#show-reference").click();
  await page.locator("#directory-query").fill("File Copier Pipeline");
  await page.getByRole("button", { name: "File Copier Pipeline" }).click();
  await expect(page.locator("#runnability-state")).toContainText("runnable · browser");
  await expect(page.locator("#run")).toBeEnabled();
  await page.locator("#run").click();
  await expect(page.locator("#result")).toContainText("Run completed", {
    timeout: 20_000,
  });
});

test("pedagogical completion is not execution evidence", async ({ page }) => {
  await gotoTour(page, "/tour/public/index.html?lesson=welcome.pull-the-cord");
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
  await gotoTour(page, "/tour/public/index.html?lesson=nodes.more-than-one-port");
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

test("typed text lesson exposes format topology and ordered evidence", async ({ page }) => {
  const { result, story } = await openTypedTextLesson(page);
  await expect(story).toContainText("std/text/format");
  await expect(story).toContainText("std/format-values/literal");
  await expect(story).toContainText("std/text/lines");
  await expect(story).toContainText("std/text/join");
  await story.getByRole("button", { name: "std/text/format" }).click();
  await expect(page.locator("#selected-node-label")).toContainText("message");
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
  const timelinePosition = page.locator("#timeline-position-label");
  const terminalPosition = await timelinePosition.innerText();

  await page.locator("#timeline-reset").click();
  await expect(timelinePosition).not.toHaveText(terminalPosition);
  const resetPosition = await timelinePosition.innerText();
  await page.locator("#timeline-step").click();
  await expect(timelinePosition).not.toHaveText(resetPosition);
  const steppedPosition = await timelinePosition.innerText();
  await story.focus();
  await page.keyboard.press("ArrowRight");
  await expect(timelinePosition).not.toHaveText(steppedPosition);
});

test("typed text lesson runs exact composition", async ({ page }) => {
  const { result } = await openTypedTextLesson(page);
  await page.locator("#scenario").selectOption("composition");
  await page.locator("#run").click();
  await expect(result).toContainText("HELLO, OPERATOR.", { timeout: 20_000 });
  await expect(page.locator('[data-id="shout"]')).toContainText("text/uppercase");
});

test("typed text lesson projects missing-value rejection", async ({ page }) => {
  const { result } = await openTypedTextLesson(page);
  await page.locator("#scenario").selectOption("missing-value");
  await page.locator("#run").click();
  await expect(result).toContainText("format/missing-value", { timeout: 20_000 });
  await expect(page.locator("#timeline-table")).toContainText(/failed|rejected/);
  await expect(page.locator("#timeline-values")).toContainText(
    "Exact run rejection: format/missing-value",
  );
});

test("typed text lesson projects cancellation", async ({ page }) => {
  const { result } = await openTypedTextLesson(page);
  await page.locator("#scenario").selectOption("cancelled");
  await page.locator("#run").click();
  await expect(result).toContainText("cancelled", { timeout: 20_000 });
  await expect(page.locator("#timeline-table")).toContainText("cancelled");
});

test("typed text lesson runs line transforms", async ({ page }) => {
  const { result } = await openTypedTextLesson(page);
  await page.locator("#scenario").selectOption("lines-join");
  await page.locator("#run").click();
  await expect(result).toContainText("alpha | beta |  | gamma", { timeout: 20_000 });
  await expect(page.locator('[data-id="lines"]')).toContainText("std/text/lines");
  await expect(page.locator('[data-id="joined"]')).toContainText("std/text/join");

  await page.locator("#scenario").selectOption("format-lines");
  await page.locator("#run").click();
  await expect(result).toContainText("alpha / beta", { timeout: 20_000 });
});

test("typed text lesson runs an edited standalone format", async ({ page }) => {
  const { result, source } = await openTypedTextLesson(page);
  await page.locator("#scenario").selectOption("standalone");
  await source.fill((await source.inputValue()).replace("operator", "robot"));
  await page.locator("#run").click();
  await expect(result).toContainText("Hello, robot.", { timeout: 20_000 });
});

test("PCM WAVE lesson runs exact codec and container providers", async ({ page }) => {
  await page.goto(
    "/tour/public/index.html?lesson=library.bounded-media-codecs",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");

  await expect(story).toBeVisible();
  for (const contract of [
    "conduit.media/wave/literal",
    "conduit.media/container/probe",
    "conduit.media/container/demux",
    "conduit.media/container/mux",
    "conduit.media/audio/decode",
    "conduit.media/audio/encode",
  ]) {
    await expect(story).toContainText(contract);
  }
  await expect(page.locator("#runnability-state")).toContainText(
    "runnable · browser",
  );
  await page.locator("#run").click();
  await expect(result).toContainText(
    "wave:pcm-s16le:48000:2:1-track:192-frames:812-bytes",
    { timeout: 20_000 },
  );
  await expect(page.locator("#evidence")).toContainText(
    '"event_kind": "terminal"',
  );
});

test("bounded spatial data runs the composed exact scan-to-grid plan", async ({ page }) => {
  await gotoTour(page,
    "/tour/public/index.html?lesson=library.bounded-spatial-data",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");

  await expect(story).toBeVisible();
  for (const contract of [
    "spatial/scan/fixture",
    "spatial/scan/transform",
    "spatial/grid/from-scan",
    "spatial/grid/inspect",
    "spatial/trajectory/fixture",
    "spatial/trajectory/inspect",
  ]) {
    await expect(story).toContainText(contract);
  }
  await expect(page.locator("#runnability-state")).toContainText(
    "runnable · browser",
  );
  await page.locator("#run").click();
  await expect(result).toContainText(
    "spatial:grid:map:2x2:occupied=2:coverage=complete",
    { timeout: 20_000 },
  );
  await expect(page.locator("#evidence")).toContainText(
    '"event_kind": "terminal"',
  );
  await expect(page.locator("#timeline-table tbody tr")).not.toHaveCount(0);
});

test("bounded spatial data runs the standalone exact grid plan", async ({ page }) => {
  await gotoTour(page,
    "/tour/public/index.html?lesson=library.bounded-spatial-data",
  );
  const result = page.locator("#result");
  await page.locator("#scenario").selectOption("spatial-grid-standalone");
  await page.locator("#run").click();
  await expect(result).toContainText(
    "spatial:grid:sensor:2x2:occupied=2:coverage=complete",
    { timeout: 20_000 },
  );
  await expect(page.locator('[data-id="grid"]')).toContainText(
    "spatial/grid/from-scan",
  );
});

test("bounded spatial data runs the exact trajectory composition", async ({ page }) => {
  await gotoTour(page,
    "/tour/public/index.html?lesson=library.bounded-spatial-data",
  );
  const result = page.locator("#result");
  await page.locator("#scenario").selectOption(
    "spatial-trajectory-text-composition",
  );
  await page.locator("#run").click();
  await expect(result).toContainText(
    "SPATIAL:TRAJECTORY:MAP:2:CLOCK/FIXTURE:LINEAR-Q30-SHORTEST",
    { timeout: 20_000 },
  );
  await expect(page.locator('[data-id="trajectory"]')).toContainText(
    "spatial/trajectory/fixture",
  );
});

test("learned inference lesson keeps model runtime and device identities exact", async ({ page }) => {
  await page.goto(
    "/tour/public/index.html?lesson=library.bounded-learned-inference",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");

  await expect(story).toBeVisible();
  for (const contract of [
    "learned/model/literal",
    "learned/tensor/literal",
    "learned/infer",
    "learned/tensor/inspect",
  ]) {
    await expect(story).toContainText(contract);
  }
  await expect(page.locator("#runnability-state")).toContainText(
    "runnable · browser",
  );
  await page.locator("#run").click();
  await expect(result).toContainText("learned:i16:1x2:[35,-3]", {
    timeout: 20_000,
  });
  await expect(page.locator("#evidence")).toContainText(
    '"event_kind": "terminal"',
  );
});

test("quick-local chat keeps one contract separate from its exact provider", async ({ page }) => {
  await gotoTour(page,
    "/tour/public/index.html?lesson=library.bounded-quick-local-chat",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");

  await expect(story).toBeVisible();
  for (const contract of ["ai/chat", "ai/chat/result/inspect"]) {
    await expect(story).toContainText(contract);
  }
  const prose = page.locator("#prose");
  for (const fact of [
    "same contract",
    "host capability",
    "resource",
    "grant",
    "exact plan",
  ]) {
    await expect(prose).toContainText(fact);
  }
  await expect(page.locator("#runnability-state")).toContainText(
    "runnable · browser",
  );
  await page.locator("#run").click();
  await expect(result).toContainText(
    "Conduit keeps contracts, implementations, host facts, plans, and evidence distinct.",
    { timeout: 20_000 },
  );
  await expect(page.locator("#evidence")).toContainText(
    '"event_kind": "terminal"',
  );
  await expect(page.locator("#timeline-table tbody tr")).not.toHaveCount(0);

  await page.locator("#scenario").selectOption(
    "quick-local-chat-result-composition",
  );
  await page.locator("#run").click();
  await expect(result).toContainText(
    "QUICK LOCAL MODEL: COMPLETED; 1 CHUNK(S); 83 BYTES; CONVERSATION CALLER-SUPPLIED-ONLY; RETENTION NONE",
    { timeout: 20_000 },
  );
  await expect(page.locator('[data-id="inspect"]')).toContainText(
    "ai/chat/result/inspect",
  );
});

test("learned lifecycle keeps evaluation separate from promotion authority", async ({ page }) => {
  await gotoTour(page,
    "/tour/public/index.html?lesson=library.bounded-learned-lifecycle",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");
  const run = page.locator("#run");

  await expect(story).toBeVisible();
  for (const contract of [
    "learned/dataset/literal",
    "learned/train",
    "learned/evaluate",
    "learned/promote",
    "learned/promotion/inspect",
  ]) {
    await expect(story).toContainText(contract);
  }
  await expect(page.locator("#runnability-state")).toContainText(
    "runnable · browser",
  );
  await run.click();
  await expect(result).toContainText(
    "learned:dataset:tiny:train:4:public",
    { timeout: 20_000 },
  );
  await expect(run).toBeEnabled({ timeout: 20_000 });

  await page.locator("#scenario").selectOption(
    "training-evaluation-without-promotion",
  );
  await run.click();
  await expect(result).toContainText(
    "learned:evaluation:accuracy@1:4/4:not-approval",
    { timeout: 20_000 },
  );
  await expect(run).toBeEnabled({ timeout: 20_000 });

  await page.locator("#scenario").selectOption(
    "authorized-promotion-composition",
  );
  await expect(result).toContainText(
    "backend-originated receipt",
    { timeout: 20_000 },
  );
  await expect(result).toContainText("CND-IMP-001");
  await expect(run).toBeDisabled();
  await expect(page.locator("#evidence")).not.toContainText(
    '"kind": "lesson-completed"',
  );
  await expect(page.locator("#timeline-table tbody tr")).toHaveCount(0);
});

test("cited claim graph keeps source support on each traversed edge", async ({ page }) => {
  await gotoTour(page,
    "/tour/public/index.html?lesson=library.bounded-knowledge-graph",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");

  await expect(story).toBeVisible();
  for (const contract of [
    "knowledge/claim/from-citation",
    "knowledge/graph/fixture",
    "knowledge/graph/query/literal",
    "knowledge/graph/traverse",
    "knowledge/graph/results/inspect",
  ]) {
    await expect(story).toContainText(contract);
  }
  await expect(page.locator("#runnability-state")).toContainText(
    "runnable · browser",
  );
  await page.locator("#run").click();
  await expect(result).toContainText(
    "knowledge:graph:Conduit--keeps-distinct-->exact-plans[source:31..42]",
    { timeout: 20_000 },
  );
  await expect(page.locator("#evidence")).toContainText(
    '"event_kind": "terminal"',
  );
  await expect(page.locator("#timeline-table tbody tr")).not.toHaveCount(0);

  await page.locator("#scenario").selectOption("graph-text-composition");
  await page.locator("#run").click();
  await expect(result).toContainText(
    "KNOWLEDGE:GRAPH:CONDUIT--KEEPS-DISTINCT-->EXACT-PLANS[SOURCE:31..42]",
    { timeout: 20_000 },
  );
  await expect(page.locator('[data-id="uppercase"]')).toContainText(
    "text/uppercase",
  );
});

test("value envelope platform lesson links checked admission to an exact run", async ({ page }) => {
  await gotoTour(page,
    "/tour/public/index.html?lesson=platform.value-envelope-clock-feedback",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");
  const source = page.locator("#source");

  await expect(story).toBeVisible();
  await expect(page.locator("#story-kind")).toHaveText("Platform contract lesson");
  await expect(story).toContainText("bounded-envelope");

  await story.getByRole("button", { name: "cycle-without-boundary" }).click();
  await expect(result).toContainText("rejected before execution with CND-FBK-002");

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
  await expect(story).toContainText("lost-ack-is-commit-unknown");

  await story.getByRole("button", { name: "wrong-holder" }).click();
  await expect(result).toContainText("rejected before execution with CND-LSE-003");

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
  await expect(story).toContainText("linux-measurement");

  await story.getByRole("button", { name: "unsupported-hard-real-time" }).click();
  await expect(result).toContainText("rejected before execution with CND-WRK-005");

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

test("cross-host lesson keeps one contract separate from its realization", async ({ page }) => {
  await gotoTour(page,
    "/tour/public/index.html?lesson=platform.cross-host-provider-conformance",
  );
  const story = page.locator("#execution-story");
  const result = page.locator("#result");
  const source = page.locator("#source");

  await expect(story).toBeVisible();
  await expect(page.locator("#story-kind")).toHaveText("Platform contract lesson");

  await story.getByRole("button", { name: "known-contract-no-provider" }).click();
  await expect(result).toContainText("rejected before execution with CND-IMP-001");
  await expect(page.locator("#run")).toBeDisabled();
  await expect(source).toHaveValue(/gain: conduit\.media\/audio\/gain/);
});

for (const scenarioId of [
  "reference-media",
  "linux-ffmpeg-process",
  "linux-sox-process",
]) {
  test(`cross-host accepted profile ${scenarioId} remains an honest checked fixture`, async ({ page }) => {
    await gotoTour(page,
      "/tour/public/index.html?lesson=platform.cross-host-provider-conformance",
    );
    const result = page.locator("#result");
    const source = page.locator("#source");
    const run = page.locator("#run");

    await page.locator("#scenario").selectOption(scenarioId);
    await expect(page.locator("#scenario")).toHaveValue(scenarioId);
    await expect(result).toContainText(`${scenarioId}: admitted by the checked contract`);
    await expect(result).toContainText("not executed by this browser host");
    await expect(run).toBeDisabled();
    await expect(source).toHaveValue(/gain: conduit\.media\/audio\/gain/);
  });
}

test("cross-host browser profile runs with the exact WASM-linked binding", async ({ page }) => {
  await gotoTour(page,
    "/tour/public/index.html?lesson=platform.cross-host-provider-conformance",
  );
  const result = page.locator("#result");
  const run = page.locator("#run");
  await page.locator("#scenario").selectOption("browser-worker");
  await expect(run).toBeEnabled();
  await run.click();
  await expect(result).toContainText("audio:s16le:48000:stereo-lr:16", {
    timeout: 20_000,
  });
  await expect(page.locator("#timeline-table tbody tr")).not.toHaveCount(0);
  await expect(page.locator("#timeline-table")).toContainText("succeeded");
  await expect(page.locator("#plan")).toContainText(
    "conduit.media/audio-gain-browser-wasm-linked",
  );
  await expect(run).toBeEnabled({ timeout: 20_000 });
});
