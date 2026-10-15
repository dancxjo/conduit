import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

async function start(page, path, transform = source => source) {
  const source = transform(await readFile(path, "utf8"));
  await page.goto("/proof/browser/browser-keymap.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  const identity = await page.evaluate(source => {
    globalThis.__conduitKeymapStart = globalThis.__conduitKeymapProof.start(source);
    return globalThis.__conduitKeymapStart;
  }, source);
  await expect(page.locator("#status")).toHaveText("listening");
  if (source.includes("input/saved-choice")) {
    await expect(page.locator("[data-saved-choice-status]")).toContainText("Saved in this browser");
  }
  if (source.includes("insertion-caret = true")) {
    await expect(page.locator("#message [data-text-value]")).toHaveText("");
    await expect(page.locator("#message [data-text-insertion-caret]")).toBeVisible();
  }
  await page.locator("#surface").focus();
  return identity;
}

async function altgr(page, code, shift = false) {
  await page.keyboard.down("AltRight");
  if (shift) await page.keyboard.down("ShiftLeft");
  await page.keyboard.press(code);
  if (shift) await page.keyboard.up("ShiftLeft");
  await page.keyboard.up("AltRight");
}

async function stop(page, activePlay) {
  const evidence = await page.evaluate(() => globalThis.__conduitKeymapProof.stop());
  expect(evidence.receipt).toMatchObject({ disposition: "cancelled", active_play_id: activePlay });
  expect(evidence.keyCount).toBeGreaterThan(0);
  expect(evidence.manifestations).toBeGreaterThan(0);
}

test("default authored US International keymap handles real browser AltGr dead keys through text/edit", async ({ page }) => {
  const failures = [];
  page.on("pageerror", error => failures.push(String(error)));
  const { activePlay } = await start(page, "plots/memory-lantern/main.conduit");
  const message = page.locator("#message [data-text-value]");
  await page.keyboard.press("Quote");
  await page.keyboard.press("Backquote");
  await page.keyboard.press("Slash");
  await expect(message).toHaveText("'`/");
  for (let index = 0; index < 3; ++index) await page.keyboard.press("Backspace");
  await expect(message).toHaveText("");

  await altgr(page, "Quote");
  await page.keyboard.press("KeyE");
  await expect(message).toHaveText("é");
  await altgr(page, "Backquote", true);
  await page.keyboard.press("Shift+KeyN");
  await expect(message).toHaveText("éÑ");

  await altgr(page, "Quote");
  await page.keyboard.press("Digit1");
  await expect(message).toHaveText("éÑ´1");
  await page.keyboard.press("Backspace");
  await expect(message).toHaveText("éÑ´");
  await page.keyboard.press("Backspace");
  await expect(message).toHaveText("éÑ");
  await altgr(page, "Quote");
  await page.keyboard.press("Backspace");
  await page.keyboard.press("KeyA");
  await expect(message).toHaveText("éÑa");
  await stop(page, activePlay);
  expect(failures).toEqual([]);
});

test("an authored layout alias and overriding layer reach the browser text/edit Face", async ({ page }) => {
  const { activePlay } = await start(page, "plots/memory-lantern/layered.conduit");
  await page.keyboard.press("KeyA");
  await expect(page.locator("#message")).toHaveText("Layered message: λ");
  await page.keyboard.press("KeyA");
  await expect(page.locator("#message")).toHaveText("Layered message: λλ");
  await stop(page, activePlay);
});


test("the six authored keyboard languages change physical mappings and restore a committed selection after reload", async ({ page }) => {
  const { activePlay } = await start(page, "plots/memory-lantern/main.conduit");
  const language = page.getByRole("combobox", { name: "Keyboard language", exact: true });
  await expect(language.locator("option")).toHaveCount(6);
  await expect(language).toHaveValue("us-intl");
  await page.keyboard.press("CapsLock");
  await language.selectOption("german");
  await expect(page.locator("[data-saved-choice-status]")).toContainText("Saved in this browser");
  await page.locator("#surface").focus();
  await page.keyboard.press("KeyY");
  await expect(page.locator("#message [data-text-value]")).toHaveText("Z");
  await page.keyboard.press("Backspace");
  await expect(page.locator("#message [data-text-value]")).toHaveText("");
  await language.selectOption("french");
  await expect(page.locator("[data-saved-choice-status]")).toContainText("Saved in this browser");
  await page.locator("#surface").focus();
  await page.keyboard.press("KeyA");
  await expect(page.locator("#message [data-text-value]")).toHaveText("Q");
  await page.keyboard.press("CapsLock");
  await page.keyboard.press("Backspace");
  await page.keyboard.press("KeyA");
  await expect(page.locator("#message [data-text-value]")).toHaveText("q");
  await stop(page, activePlay);

  const restored = await start(page, "plots/memory-lantern/main.conduit");
  await expect(page.getByRole("combobox", { name: "Keyboard language", exact: true })).toHaveValue("french");
  await page.keyboard.press("KeyA");
  await expect(page.locator("#message [data-text-value]")).toHaveText("q");
  await stop(page, restored.activePlay);
});

test("the shared textual Face renders an end insertion caret with responsive and accessible Look", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1100, height: 800 });
  const { activePlay } = await start(page, "plots/memory-lantern/main.conduit");
  await page.keyboard.press("Shift+KeyC");
  await page.keyboard.press("KeyA");
  await page.keyboard.press("KeyF");
  await altgr(page, "Quote");
  await page.keyboard.press("KeyE");
  await page.keyboard.press("Space");
  await altgr(page, "Backquote", true);
  await page.keyboard.press("Shift+KeyN");
  const text = page.locator("#message [data-text-value]");
  const caret = page.locator("#message [data-text-insertion-caret]");
  await expect(text).toHaveText("Café Ñ");
  await expect(page.locator("#message")).toHaveAttribute("data-insertion-caret", "end");
  await expect(caret).toHaveAttribute("aria-hidden", "true");
  await expect(caret).toBeVisible();
  await expect(caret).toHaveCSS("animation-name", "conduit-caret");
  await page.getByRole("combobox", { name: "Keyboard language" }).focus();
  await expect(caret).toHaveCSS("animation-name", "none");
  await expect(text).toHaveText("Café Ñ");
  await page.locator("#surface").focus();
  await expect(caret).toHaveCSS("animation-name", "conduit-caret");
  await expect(page.getByRole("combobox", { name: "Keyboard language" })).toBeEnabled();
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(caret).toHaveCSS("animation-name", "none");
  await expect(page.locator("#message")).toHaveCSS("transition-duration", "0s");
  await page.screenshot({ path: testInfo.outputPath("memory-lantern-desktop.png"), fullPage: true });

  await page.setViewportSize({ width: 360, height: 800 });
  await page.evaluate(() => { document.documentElement.style.zoom = "2"; });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
  await expect(text).toHaveText("Café Ñ");
  await expect(caret).toBeVisible();
  await expect(page.locator("output[data-saved-choice-status]")).toHaveText("Saved in this browser — English — US International");
  await expect(page.locator("output[data-saved-choice-status]")).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("memory-lantern-narrow-200-percent.png"), fullPage: true });
  await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
  await expect(caret).toHaveCSS("forced-color-adjust", "none");
  await expect(page.locator("#message")).toHaveCSS("box-shadow", "none");
  await expect(text).toHaveText("Café Ñ");
  await stop(page, activePlay);
});


test("authored multi-scalar keymap text larger than four bytes reaches retained text intact", async ({ page }) => {
  const { activePlay } = await start(page, "plots/memory-lantern/layered.conduit", source => {
    expect(source).toContain('action: text("λ")');
    return source.replace('action: text("λ")', 'action: text("λ🌱!")');
  });
  await page.keyboard.press("KeyA");
  await expect(page.locator("#message")).toHaveText("Layered message: λ🌱!");
  await stop(page, activePlay);
});

test("two composed keymap gears in one Plot keep pending dead keys independent under physical input fanout", async ({ page }) => {
  const source = await readFile("proof/fixtures/plots/two-keymaps.conduit", "utf8");
  await page.goto("/proof/browser/browser-keymap.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  await page.evaluate(() => {
    globalThis.twoKeymapFaces = {};
    // Read actual rendered Face updates. The shared renderer replaces one
    // text node per named manifestation; records retain both nodes even when
    // two kernel outputs are manifested in the same browser task.
    globalThis.twoKeymapObserver = new MutationObserver(records => {
      for (const record of records) for (const node of record.addedNodes) {
        if (node.nodeType !== Node.TEXT_NODE) continue;
        const named = /^([^:]+): ([\s\S]*)$/.exec(node.textContent);
        if (named) globalThis.twoKeymapFaces[named[1]] = named[2];
      }
    });
    globalThis.twoKeymapObserver.observe(document.querySelector("#message"), { childList: true });
  });
  const { activePlay } = await page.evaluate(source => {
    globalThis.twoKeymapStart = globalThis.__conduitKeymapProof.start(source);
    return globalThis.twoKeymapStart;
  }, source);
  const faces = () => page.evaluate(() => globalThis.twoKeymapFaces);
  await expect.poll(faces).toEqual({ "Acute layout": "", "Grave layout": "" });
  await page.locator("#surface").focus();
  await altgr(page, "Quote");
  await expect.poll(faces).toEqual({ "Acute layout": "", "Grave layout": "'" });
  await page.keyboard.press("KeyE");
  await expect.poll(faces).toEqual({ "Acute layout": "é", "Grave layout": "'e" });
  await altgr(page, "Backquote");
  await expect.poll(faces).toEqual({ "Acute layout": "é`", "Grave layout": "'e" });
  await page.keyboard.press("KeyA");
  await expect.poll(faces).toEqual({ "Acute layout": "é`a", "Grave layout": "'eà" });
  await stop(page, activePlay);
  await page.evaluate(() => globalThis.twoKeymapObserver.disconnect());
});
