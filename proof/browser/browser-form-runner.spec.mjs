import { expect, test } from "@playwright/test";
import { openTourStep, startTour } from "./book-test-server.mjs";

const FORM = `form text-chain {
    source: text/literal("hello")
    prefix: text/join("say: ")
    upper: text/upper
    result: presentation/text

    source > prefix > upper > result
}`;

let entrance;

test.beforeEach(async () => { entrance = await startTour(); });
test.afterEach(() => entrance?.child.kill());

test("Tour and a normal browser Form session use one installed repertoire and equivalent semantics", async ({ page }) => {
  await openTourStep(page, entrance, 0);
  const runtimeUrl = new URL("runtime.wasm", entrance.url).href;
  const runner = page.locator('[data-application-component="tour-laboratory"]');
  await runner.locator("textarea").fill(FORM);
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("SAY: HELLO");
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");
  const tourEvidence = await runner.locator(".exact-evidence").textContent();

  const normal = await page.evaluate(async ({ source, runtimeUrl }) => {
    const bytes = new Uint8Array(await (await fetch(runtimeUrl)).arrayBuffer());
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const api = instance.exports;
    const required = [
      "memory", "conduit_browser_form_input_ptr", "conduit_browser_form_input_capacity",
      "conduit_browser_form_output_ptr", "conduit_browser_form_output_len",
      "conduit_browser_form_inventory", "conduit_browser_form_admit_source_interaction",
      "conduit_browser_form_start", "conduit_browser_form_complete", "conduit_browser_form_cancel",
    ];
    if (required.some((name) => !(name in api))) throw new Error("normal browser Form ABI is incomplete");
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    const read = () => JSON.parse(decoder.decode(new Uint8Array(
      api.memory.buffer, api.conduit_browser_form_output_ptr(), api.conduit_browser_form_output_len(),
    )));
    if (api.conduit_browser_form_inventory() < 0) throw new Error("normal inventory refused");
    const inventory = read();
    const sourceBytes = encoder.encode(source);
    const hostBytes = encoder.encode("browser/ordinary-application");
    const bootBytes = encoder.encode("browser-boot/ordinary-application");
    new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), sourceBytes.length).set(sourceBytes);
    if (api.conduit_browser_form_admit_source_interaction(sourceBytes.length, 41n) < 0) throw new Error("source admission refused");
    const total = hostBytes.length + bootBytes.length + sourceBytes.length;
    const input = new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), total);
    input.set(hostBytes); input.set(bootBytes, hostBytes.length); input.set(sourceBytes, hostBytes.length + bootBytes.length);
    if (api.conduit_browser_form_start(hostBytes.length, bootBytes.length, sourceBytes.length, 7n) < 0) throw new Error("normal Form start refused");
    const effect = read();
    if (api.conduit_browser_form_complete() < 0) throw new Error("normal Form completion refused");
    return { inventory, effect, receipt: read(), runtimeBytes: bytes.byteLength };
  }, { source: FORM, runtimeUrl });

  expect(normal.effect).toMatchObject({
    effect_kind: "manifestation",
    text: "SAY: HELLO",
    host_id: "browser/ordinary-application",
    boot_id: "browser-boot/ordinary-application",
  });
  expect(normal.effect.expanded_gears.map(({ implementation_id }) => implementation_id).sort()).toEqual([
    "browser/kernel-text-literal@1", "browser/kernel-text-join@1",
    "browser/kernel-text-upper@1", "browser/presentation-text@1",
  ].sort());
  expect(normal.receipt).toMatchObject({ disposition: "completed", manifestation_completions: 1 });
  expect(tourEvidence).toContain("browser/kernel-text-upper@1");
  expect(tourEvidence).not.toContain(normal.effect.active_play_id);
  expect(new Set(normal.inventory.entries.map(({ family }) => family)).size).toBeGreaterThanOrEqual(6);
  expect(normal.inventory.entries.every(({ implementation_id, artifact_id }) => implementation_id && artifact_id)).toBe(true);
  expect(normal.runtimeBytes).toBeGreaterThan(0);
});

test("a normal browser Form session refuses an unsupported semantic Kind before Play", async ({ page }) => {
  await page.goto(entrance.url);
  const refusal = await page.evaluate(async ({ runtimeUrl }) => {
    const { instance } = await WebAssembly.instantiate(await (await fetch(runtimeUrl)).arrayBuffer(), {});
    const api = instance.exports;
    const source = new TextEncoder().encode(`form missing-installation {
      source: text/literal("hello")
      result: presentation/text
      unavailable: layout/inset
      source > result
    }`);
    const host = new TextEncoder().encode("browser/ordinary-negative");
    const boot = new TextEncoder().encode("browser-boot/ordinary-negative");
    const ptr = api.conduit_browser_form_input_ptr();
    new Uint8Array(api.memory.buffer, ptr, source.length).set(source);
    api.conduit_browser_form_admit_source_interaction(source.length, 1n);
    const input = new Uint8Array(api.memory.buffer, ptr, host.length + boot.length + source.length);
    input.set(host); input.set(boot, host.length); input.set(source, host.length + boot.length);
    const code = api.conduit_browser_form_start(host.length, boot.length, source.length, 1n);
    const output = JSON.parse(new TextDecoder().decode(new Uint8Array(
      api.memory.buffer, api.conduit_browser_form_output_ptr(), api.conduit_browser_form_output_len(),
    )));
    return { code, output };
  }, { runtimeUrl: new URL("runtime.wasm", entrance.url).href });
  expect(refusal.code).toBeLessThan(0);
  expect(refusal.output).toMatchObject({ disposition: "refused-before-play", category: "missing-implementation-or-placement" });
  expect(refusal.output.message).toContain("layout/inset");
});

test("an authored five-transition button stream handles three ordinary browser presses", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(String(error)));
  await openTourStep(page, entrance, 0);
  const runner = page.locator('[data-application-component="tour-laboratory"]');
  await runner.locator("textarea").fill(`form three-presses {
    button: input/button(maximum-transitions = 5)
    state: input/button-indicator-state
    indicator: presentation/indicator-state
    button > state > indicator
  }`);
  await runner.getByRole("button", { name: "Run" }).click();
  const control = runner.getByRole("button", { name: "Hold to control indicator" });
  await expect(control).toBeVisible();
  const bounds = await control.boundingBox();
  await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
  try {
    await page.mouse.down();
    await page.mouse.up();
    await page.mouse.down();
    await page.mouse.up();
    await page.mouse.down();
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("5 planned manifestations");
    await expect(runner.locator('[role="img"]')).toHaveAttribute("aria-label", "Indicator on");
    expect(failures).toEqual([]);
  } finally {
    await page.mouse.up();
  }
});

test("button input progresses alongside a pending timer and the Play can be cancelled", async ({ page }) => {
  await openTourStep(page, entrance, 0);
  const runner = page.locator('[data-application-component="tour-laboratory"]');
  await runner.locator("textarea").fill(`form concurrent {
    button: input/button(maximum-transitions = 1)
    state: input/button-indicator-state
    indicator: presentation/indicator-state
    clock: time/every(freq = 10000ms)
    count: state/count(start = 0)
    show: presentation/count(maximum-values = 5)
    button > state > indicator
    clock.tick > count.bump
    count.value > show.value
  }`);
  await runner.getByRole("button", { name: "Run", exact: true }).click();
  const control = runner.getByRole("button", { name: "Hold to control indicator" });
  await expect(control).toBeVisible();
  await control.hover();
  await page.mouse.down();
  await expect(runner.getByRole("img", { name: "Indicator on", exact: true })).toBeVisible({ timeout: 3000 });
  await page.mouse.up();
  await runner.getByRole("button", { name: "Stop", exact: true }).click();
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText("cancelled");
  await expect(runner.getByRole("img", { name: "Indicator off", exact: true })).toBeVisible();
  await runner.locator("textarea").fill(FORM);
  await runner.getByRole("button", { name: "Run", exact: true }).click();
  await expect(runner.locator(".morse")).toHaveText("SAY: HELLO");
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");
});
