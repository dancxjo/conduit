import { expect, test } from "@playwright/test";

test("document-wide input leaves native controls operable without an admitted key request", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  const control = page.getByRole("button", { name: "Native control" });
  await control.focus();
  await control.press("Enter");
  await expect(page.locator("#native-actions")).toHaveText("1");
});

test("ordered button transitions survive gaps between Host requests and refuse overflow", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  await page.evaluate(() => {
    globalThis.buttonFirst = globalThis.__conduitHumanInput.adapter.nextButton();
  });
  const bounds = await page.locator("#surface").boundingBox();
  await page.mouse.move(bounds.x + 20, bounds.y + 20);
  await page.mouse.down();
  await page.mouse.up();
  const transitions = await page.evaluate(async () => [
    await globalThis.buttonFirst,
    await globalThis.__conduitHumanInput.adapter.nextButton(),
  ]);
  expect(transitions.map(({ pressed, sequence }) => [pressed, sequence])).toEqual([[true, 0], [false, 1]]);
  for (let index = 0; index < 5; index += 1) {
    await page.mouse.down();
    await page.mouse.up();
  }
  expect(await page.evaluate(() => globalThis.__conduitHumanInput.adapter.nextButton().catch((error) => error.code))).toBe("Pressure");
  const key = page.evaluate(() => globalThis.__conduitHumanInput.adapter.nextKeyboard());
  await page.locator("#surface").focus();
  await page.keyboard.down("KeyA");
  expect(Array.from((await key).canonical_bytes)).toEqual([0x04, 0, 0]);
  await page.keyboard.up("KeyA");
});

test("queued button release cannot survive a replacement Boot", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  await page.evaluate(() => { globalThis.buttonFirst = globalThis.__conduitHumanInput.adapter.nextButton(); });
  const bounds = await page.locator("#surface").boundingBox();
  await page.mouse.move(bounds.x + 20, bounds.y + 20);
  await page.mouse.down();
  await page.mouse.up();
  expect(await page.evaluate(async () => (await globalThis.buttonFirst).pressed)).toBe(true);
  await page.evaluate(() => globalThis.__conduitHumanInput.staleBoot());
  expect(await page.evaluate(() => globalThis.__conduitHumanInput.adapter.nextButton().catch((error) => error.code))).toBe("StaleBoot");
});

test("selected keyboard and pointer adapt real Chromium actions to portable values", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  const surface = page.locator("#surface");
  await surface.focus();
  const pendingKey = page.evaluate(() => globalThis.__conduitHumanInput.acquireKeyboard());
  await page.keyboard.down("Shift");
  await page.keyboard.press("KeyA");
  await page.keyboard.up("Shift");
  await pendingKey;
  await expect(page.locator("#keyboard")).toContainText('"schema":"input/key-event@1"');
  const keyboard = JSON.parse(await page.locator("#keyboard").textContent());
  expect(keyboard.canonical_bytes).toEqual([0xe1, 0, 2]);
  expect(keyboard.owner).toMatchObject({
    host_id: "browser/human-input-proof",
    boot_id: "browser/human-input-proof-boot",
    offer_generation: 1,
  });

  const bounds = await surface.boundingBox();
  await page.mouse.move(bounds.x + 100, bounds.y + 50);
  await page.mouse.down();
  await page.mouse.up();
  await expect(page.locator("#pointer")).toContainText('"schema":"input/pointer-event@1"');
  const pointer = JSON.parse(await page.locator("#pointer").textContent());
  expect(pointer).toMatchObject({
    position_x: 250000,
    position_y: 250000,
    primary_pressed: false,
    coalesced: 0,
    dropped: 0,
    queue_capacity: 1,
    sequence: 2,
  });
  expect(failures).toEqual([]);
});

test("ordered keyboard transitions survive gaps between Host requests and refuse overflow", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  await page.locator("#surface").focus();

  await page.keyboard.press("KeyA");
  const queued = await page.evaluate(async () => [
    Array.from((await globalThis.__conduitHumanInput.adapter.nextKeyboard()).canonical_bytes),
    Array.from((await globalThis.__conduitHumanInput.adapter.nextKeyboard()).canonical_bytes),
  ]);
  expect(queued).toEqual([[0x04, 0, 0], [0x04, 1, 0]]);

  for (const key of ["KeyB", "KeyC", "KeyD", "KeyE", "KeyF"]) await page.keyboard.press(key);
  expect(await page.evaluate(() => globalThis.__conduitHumanInput.adapter.nextKeyboard().catch((error) => error.code))).toBe("Pressure");
  const before = await page.evaluate(() => globalThis.__conduitHumanInput.pointerValues.length);
  const bounds = await page.locator("#surface").boundingBox();
  await page.mouse.move(bounds.x + 300, bounds.y + 100);
  await expect.poll(() => page.evaluate(() => globalThis.__conduitHumanInput.pointerValues.length)).toBeGreaterThan(before);
});

test("profile omission, finite pressure, cancellation, and stale Boot stay distinct", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html?profile=viewer");
  await expect(page.locator("#status")).toHaveText("ready");
  expect(await page.evaluate(async () => (await globalThis.__conduitHumanInput.acquireKeyboard()).code)).toBe("UnsupportedInput");
  await expect(page.locator("#pointer")).toHaveText("UnsupportedInput");

  await page.goto("/proof/browser/browser-human-input.test.html");
  const outcomes = await page.evaluate(async () => {
    const requests = Array.from({ length: 9 }, () => globalThis.__conduitHumanInput.adapter.nextKeyboard().catch((error) => error.code));
    const pressure = await requests[8];
    globalThis.__conduitHumanInput.adapter.cancelPending();
    return { pressure, cancelled: await requests[0] };
  });
  expect(outcomes).toEqual({ pressure: "Pressure", cancelled: "Cancelled" });

  await page.evaluate(() => globalThis.__conduitHumanInput.staleBoot());
  const pending = page.evaluate(() => globalThis.__conduitHumanInput.acquireKeyboard());
  await page.locator("#surface").focus();
  await page.keyboard.press("KeyB");
  await pending;
  await expect(page.locator("#keyboard")).toHaveText("StaleBoot");
});

test("focus loss is recoverable while page loss is terminal", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  const focusPending = page.evaluate(() => globalThis.__conduitHumanInput.acquireKeyboard());
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await focusPending;
  await expect(page.locator("#keyboard")).toHaveText("FocusLost");

  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  const pagePending = page.evaluate(() => globalThis.__conduitHumanInput.acquireKeyboard());
  await page.evaluate(() => window.dispatchEvent(new Event("pagehide")));
  await pagePending;
  await expect(page.locator("#keyboard")).toHaveText("PageLost");
  expect(await page.evaluate(async () => (await globalThis.__conduitHumanInput.acquireKeyboard()).code)).toBe("PageLost");
});

test("Body routing reuses one pointer slot for 100,000 browser observations", async ({ page }) => {
  await page.goto("/proof/browser/browser-body-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  const result = await page.evaluate(() => globalThis.__conduitBodyInput.stress(100_000));
  expect(result.first).toBe(0);
  expect(result.latest.sequence).toBe(99_999);
  expect(result.latest.queue_capacity).toBe(1);
  expect(result.latest.coalesced).toBe(99_998);
  expect(result.pressure).toEqual([expect.objectContaining({
    kind: "pointer",
    plot: "plot/pointer-pressure-proof",
    capacity: 1,
    occupancy: 0,
    accepted: 100_000,
    delivered: 2,
    coalesced: 99_998,
    refusals: 0,
    terminal: null,
  })]);
  await page.evaluate(() => globalThis.__conduitBodyInput.close());
});


test("physical keyboard positions include punctuation, navigation, keypad and sided modifiers", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  await page.locator("#surface").focus();
  for (const [key, usage] of [["Quote", 0x34], ["Backquote", 0x35], ["BracketLeft", 0x2f],
    ["Backslash", 0x31], ["Slash", 0x38], ["ArrowLeft", 0x50], ["NumpadEnter", 0x58],
    ["Numpad7", 0x5f], ["F12", 0x45]]) {
    const expectedUsage = usage;
    await page.keyboard.press(key);
    expect(await page.evaluate(async () => [
      [...(await globalThis.__conduitHumanInput.adapter.nextKeyboard()).canonical_bytes],
      [...(await globalThis.__conduitHumanInput.adapter.nextKeyboard()).canonical_bytes],
    ])).toEqual([[expectedUsage, 0, 0], [expectedUsage, 1, 0]]);
  }
  for (const [key, usage, bits] of [["ControlLeft", 0xe0, 1], ["ShiftLeft", 0xe1, 2],
    ["AltLeft", 0xe2, 4], ["MetaLeft", 0xe3, 8], ["ControlRight", 0xe4, 16],
    ["ShiftRight", 0xe5, 32], ["AltRight", 0xe6, 64], ["MetaRight", 0xe7, 128]]) {
    await page.keyboard.down(key);
    await page.keyboard.press("KeyA");
    await page.keyboard.up(key);
    expect(await page.evaluate(async () => Promise.all(Array.from({ length: 4 }, async () =>
      [...(await globalThis.__conduitHumanInput.adapter.nextKeyboard()).canonical_bytes]))))
      .toEqual([[usage, 0, bits], [4, 0, bits], [4, 1, bits], [usage, 1, 0]]);
  }
});

test("AltGraph aggregate flags never fabricate left control or left alt; duplicate and repeat events are not transitions", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  expect(await page.evaluate(async () => {
    const surface = document.querySelector("#surface");
    const send = (type, code, options = {}) => surface.dispatchEvent(new KeyboardEvent(type, {
      code, bubbles: true, cancelable: true, ...options,
    }));
    send("keyup", "KeyA"); // no corresponding physical press
    send("keydown", "AltRight", { key: "AltGraph", ctrlKey: true, altKey: true });
    send("keydown", "KeyA", { ctrlKey: true, altKey: true });
    send("keydown", "KeyA", { ctrlKey: true, altKey: true });
    send("keydown", "KeyA", { repeat: true, ctrlKey: true, altKey: true });
    send("keyup", "KeyA", { ctrlKey: true, altKey: true });
    send("keyup", "AltRight");
    return Promise.all(Array.from({ length: 4 }, async () =>
      [...(await globalThis.__conduitHumanInput.adapter.nextKeyboard()).canonical_bytes]));
  })).toEqual([[0xe6, 0, 64], [4, 0, 64], [4, 1, 64], [0xe6, 1, 0]]);
  await page.locator("#surface").focus();
  await page.keyboard.press("KeyB");
  expect(await page.evaluate(async () => [...(await globalThis.__conduitHumanInput.adapter.nextKeyboard()).canonical_bytes]))
    .toEqual([5, 0, 0]);
});

async function documentKeyboard(page) {
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  await page.evaluate(async () => {
    globalThis.__conduitHumanInput.adapter.close();
    const { openBrowserHumanInput } = await import("/targets/browser/host/assets/browser-human-input.mjs");
    const boot = { host_id: "browser/document-input-proof", boot_id: "browser/document-input-boot",
      implementation_registry: ["browser/keyboard-events@1"] };
    globalThis.documentKeyboard = openBrowserHumanInput({ target: document, boot });
    const select = document.createElement("select");
    select.id = "native-select";
    select.setAttribute("aria-label", "Native choice");
    for (const value of ["first", "second", "third"]) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = value;
      select.append(option);
    }
    select.addEventListener("keyup", event => { globalThis.nativeKeyupPrevented = event.defaultPrevented; });
    document.querySelector("#surface").before(select);
    globalThis.plotKeyDelivered = false;
    globalThis.plotKey = globalThis.documentKeyboard.nextKeyboard().then(value => {
      globalThis.plotKeyDelivered = true;
      return [...value.canonical_bytes];
    });
  });
}

test("standing document keyboard requests preserve native select, Tab and button keyboard actions", async ({ page }) => {
  await documentKeyboard(page);
  const choice = page.getByRole("combobox", { name: "Native choice" });
  await choice.focus();
  await page.keyboard.press("ArrowDown");
  await expect(choice).toHaveValue("second");
  await page.keyboard.press("Tab");
  await expect(page.locator("#surface")).toBeFocused();
  const button = page.getByRole("button", { name: "Native control" });
  await button.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#native-actions")).toHaveText("1");
  expect(await page.evaluate(() => globalThis.plotKeyDelivered)).toBe(false);
  await page.locator("#surface").focus();
  await page.keyboard.press("KeyA");
  expect(await page.evaluate(() => globalThis.plotKey)).toEqual([4, 0, 0]);
  expect(await page.evaluate(async () => [...(await globalThis.documentKeyboard.nextKeyboard()).canonical_bytes]))
    .toEqual([4, 1, 0]);
  await page.evaluate(() => globalThis.documentKeyboard.close());
});

test("physical modifiers remain exact across native-control boundaries without leaking native key presses", async ({ page }) => {
  await documentKeyboard(page);
  const choice = page.getByRole("combobox", { name: "Native choice" });
  await choice.focus();
  await page.keyboard.down("AltRight");
  await page.locator("#surface").focus();
  await page.keyboard.press("KeyA");
  await page.keyboard.up("AltRight");
  await page.keyboard.press("KeyB");
  expect(await page.evaluate(async () => [await globalThis.plotKey,
    ...await Promise.all(Array.from({ length: 3 }, async () =>
      [...(await globalThis.documentKeyboard.nextKeyboard()).canonical_bytes])),
  ])).toEqual([[4, 0, 64], [4, 1, 64], [5, 0, 0], [5, 1, 0]]);

  await page.keyboard.down("ShiftLeft");
  await choice.focus();
  await page.keyboard.up("ShiftLeft");
  expect(await page.evaluate(async () => Promise.all(Array.from({ length: 2 }, async () =>
    [...(await globalThis.documentKeyboard.nextKeyboard()).canonical_bytes]))))
    .toEqual([[0xe1, 0, 2], [0xe1, 1, 0]]);
  expect(await page.evaluate(() => globalThis.nativeKeyupPrevented)).toBe(false);
  await page.keyboard.press("ArrowDown");
  await expect(choice).toHaveValue("second");
  await page.evaluate(() => globalThis.documentKeyboard.close());
});

test("element-scoped physical input leaves its native DOM editor descendant authoritative", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  await page.evaluate(() => {
    const editor=document.createElement("textarea");
    editor.setAttribute("aria-label","Native editor");
    document.querySelector("#surface").append(editor);
    globalThis.elementPlotKeyDelivered=false;
    globalThis.elementPlotKey=globalThis.__conduitHumanInput.acquireKeyboard().then(value => {
      globalThis.elementPlotKeyDelivered=true; return [...value.canonical_bytes];
    });
  });
  await page.getByRole("textbox",{name:"Native editor"}).focus();
  await page.keyboard.type("cafe");
  await expect(page.getByRole("textbox",{name:"Native editor"})).toHaveValue("cafe");
  expect(await page.evaluate(() => globalThis.elementPlotKeyDelivered)).toBe(false);
  await page.locator("#surface").focus();
  await page.keyboard.press("KeyA");
  expect(await page.evaluate(() => globalThis.elementPlotKey)).toEqual([4,0,0]);
  await page.evaluate(() => globalThis.__conduitHumanInput.adapter.close());
});
