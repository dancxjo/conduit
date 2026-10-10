import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { writeFile } from "node:fs/promises";
const fixture = readFileSync(new URL("./fixtures/native-value-window.conduit", import.meta.url), "utf8");

for (const [name, sourceText, accepted] of [
  ["computed window", fixture, true],
  ["equivalent checked arguments", fixture.replace("Window<2, 3>", "Window<1 + 1, 3>"), true],
  ["wrong history count", fixture.replace("history: [[1,2,3],[4,5,6]]", "history: [[1,2,3]]"), false],
  ["checked argument overflow", fixture.replace("Window<2, 3>", "Window<65535 + 1, 3>"), false],
]) {
  test(`native value Types execute ${name} through the browser kernel`, async ({ page }, testInfo) => {
    test.setTimeout(60_000);
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto("/proof/browser/native-value-types.test.html");
    const sources = [{ name, source: sourceText, accepted }];
    const evidence = await page.evaluate(async (sources) => {
      const response = await fetch("/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm");
      if (!response.ok) throw new Error(`WASM fetch failed: ${response.status}`);
      const wasm = await response.arrayBuffer();
      const { instance } = await WebAssembly.instantiate(wasm, {});
      const api = instance.exports;
      const encoder = new TextEncoder();
      const decoder = new TextDecoder();
      const read = () => JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer, api.conduit_browser_plot_output_ptr(), api.conduit_browser_plot_output_len())));
      const write = (...parts) => {
        const count = parts.reduce((sum, part) => sum + part.length, 0);
        if (count > api.conduit_browser_plot_input_capacity()) throw new Error("proof input exceeds ABI bound");
        const input = new Uint8Array(api.memory.buffer, api.conduit_browser_plot_input_ptr(), count);
        let offset = 0;
        for (const part of parts) { input.set(part, offset); offset += part.length; }
      };
      const rows = [];
      for (const [index, request] of sources.entries()) {
        const bytes = encoder.encode(request.source);
        const host = encoder.encode("browser/native-value-proof");
        const boot = encoder.encode("boot/native-value-proof");
        write(bytes);
        if (api.conduit_browser_plot_admit_source_interaction(bytes.length, BigInt(index + 1)) < 0) throw new Error(`source interaction: ${JSON.stringify(read())}`);
        write(host, boot, bytes);
        const status = api.conduit_browser_plot_start(host.length, boot.length, bytes.length, BigInt(index + 1));
        const effect = read();
        if (!request.accepted) {
          if (status >= 0 || effect.disposition !== "refused-before-play") throw new Error(`invalid Source reached Play: ${JSON.stringify(effect)}`);
          rows.push({ name: request.name, source: request.source, status, refusal: effect });
          continue;
        }
        if (status < 0) throw new Error(`native value: ${JSON.stringify(effect)}`);
        if (effect.effect_kind !== "manifestation" || effect.presentation_kind !== "presentation/bool-value" || effect.text !== "true") throw new Error(`native value: ${JSON.stringify(effect)}`);
        document.querySelector("#result").textContent = effect.text;
        const play = encoder.encode(effect.active_play_id);
        const placement = encoder.encode(effect.placement_id);
        write(play, placement);
        if (api.conduit_browser_plot_complete_effect(play.length, placement.length, effect.observation_sequence, 0) < 0) throw new Error(`effect acknowledgment: ${JSON.stringify(read())}`);
        const receipt = read();
        if (receipt.disposition !== "completed" || receipt.active_play_id !== effect.active_play_id) throw new Error(`completion: ${JSON.stringify(receipt)}`);
        rows.push({ name: request.name, source: request.source, effect, receipt });
      }
      const digest = await crypto.subtle.digest("SHA-256", wasm);
      return { rows, wasm_sha256: [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join(""), browser: navigator.userAgent };
    }, sources);
    expect(errors).toEqual([]);
    expect(evidence.rows).toHaveLength(1);
    if (accepted) await expect(page.locator("#result")).toHaveText("true");
    const evidencePath = testInfo.outputPath("native-value-browser-evidence.json");
    await writeFile(evidencePath, JSON.stringify(evidence, null, 2));
    await testInfo.attach("native-value-browser-evidence.json", { path: evidencePath, contentType: "application/json" });
  });
}
