import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

const cases = [
  ["morse-network", "SOS"],
  ["memory-lantern", "READY"],
  ["desk-telegraph", "CALLING"],
];

for (const [slug, expectedText] of cases) {
  test(`reviewed Form ${slug} runs browser-safe`, async ({ page }) => {
    const source = await readFile(`forms/${slug}/main.conduit`, "utf8");
    await page.goto("/");
    const evidence = await page.evaluate(async ({ source, slug }) => {
      const response = await fetch("/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm");
      if (!response.ok) throw new Error(`browser runtime fetch failed: ${response.status}`);
      const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), {});
      const api = instance.exports;
      const encoder = new TextEncoder();
      const decoder = new TextDecoder();
      const read = () => JSON.parse(decoder.decode(new Uint8Array(
        api.memory.buffer,
        api.conduit_browser_form_output_ptr(),
        api.conduit_browser_form_output_len(),
      )));
      const sourceBytes = encoder.encode(source);
      const hostBytes = encoder.encode(`browser/conformance/${slug}`);
      const bootBytes = encoder.encode(`browser-boot/conformance/${slug}`);
      const inputPointer = api.conduit_browser_form_input_ptr();
      new Uint8Array(api.memory.buffer, inputPointer, sourceBytes.length).set(sourceBytes);
      if (api.conduit_browser_form_admit_source_interaction(sourceBytes.length, 1n) < 0) {
        throw new Error(`source interaction refused: ${JSON.stringify(read())}`);
      }
      const input = new Uint8Array(
        api.memory.buffer,
        inputPointer,
        hostBytes.length + bootBytes.length + sourceBytes.length,
      );
      input.set(hostBytes);
      input.set(bootBytes, hostBytes.length);
      input.set(sourceBytes, hostBytes.length + bootBytes.length);
      if (api.conduit_browser_form_start(hostBytes.length, bootBytes.length, sourceBytes.length, 1n) < 0) {
        throw new Error(`Form start refused: ${JSON.stringify(read())}`);
      }
      const effect = read();
      if (api.conduit_browser_form_complete() < 0) {
        throw new Error(`Form completion refused: ${JSON.stringify(read())}`);
      }
      return { effect, receipt: read() };
    }, { source, slug });

    expect(evidence.effect).toMatchObject({
      effect_kind: "manifestation",
      text: expectedText,
    });
    expect(evidence.receipt).toMatchObject({
      disposition: "completed",
      active_play_id: evidence.effect.active_play_id,
    });
    console.log(`CONDUIT_FORM_EVIDENCE=${JSON.stringify({
      plan_id: evidence.effect.plan_id,
      play_id: evidence.effect.active_play_id,
    })}`);
  });
}
