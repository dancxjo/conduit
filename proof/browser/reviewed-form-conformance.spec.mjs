import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

const cases = [
  ["morse-network", { keys: "a", presentationKind: "presentation/indicator", morseSegments: true }],
  ["memory-lantern", { keys: "ready", presentationKind: "presentation/text", text: "ready" }],
  ["desk-telegraph", { keys: "calling\n", presentationKind: "presentation/text", text: "calling" }],
  ["night-radio", { keys: "night report\n", presentationKind: "presentation/text", text: "night report" }],
];
const selectedCases = new Set(JSON.parse(process.env.CONDUIT_FORM_CASES_JSON ?? "null") ?? cases.map(([slug]) => `reviewed Form ${slug} runs browser-safe`));

for (const [slug, expected] of cases) {
  const caseName = `reviewed Form ${slug} runs browser-safe`;
  test(caseName, async ({ page }) => {
    test.skip(!selectedCases.has(caseName), "not selected by the admitted Form batch");
    try {
      const source = await readFile(`forms/${slug}/main.conduit`, "utf8");
      await page.goto("/");
      const evidence = await page.evaluate(async ({ source, slug, expected }) => {
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
      let progress = read();
      const playId = progress.active_play_id;
      let effect = null;
      const keys = [...expected.keys];
      const keyBytes = (key) => {
        if (key === "\n") return new Uint8Array([0x28, 0, 0]);
        if (key === " ") return new Uint8Array([0x2c, 0, 0]);
        const usage = key.codePointAt(0) - "a".codePointAt(0) + 0x04;
        if (usage < 0x04 || usage > 0x1d) throw new Error(`unsupported scripted key ${key}`);
        return new Uint8Array([usage, 0, 0]);
      };
      for (let step = 0; step < 128 && !effect; step += 1) {
        if (!progress.effect_kind) {
          if (progress.disposition !== "waiting") throw new Error(`Play stopped before manifestation: ${JSON.stringify(progress)}`);
          if (api.conduit_browser_form_poll_effect() < 0) throw new Error("effect poll refused");
          progress = read();
          continue;
        }
        let output = new Uint8Array();
        if (progress.effect_kind === "key-event") {
          const key = keys.shift();
          if (key === undefined) {
            if (api.conduit_browser_form_poll_effect() < 0) throw new Error("effect poll refused while presentation was pending");
            progress = read();
            continue;
          }
          output = keyBytes(key);
        } else if (progress.effect_kind === "manifestation") {
          const matches = expected.text
            ? progress.text === expected.text
            : progress.segments?.length > 0;
          if (matches) effect = progress;
        } else {
          throw new Error(`unexpected browser Host effect ${progress.effect_kind}`);
        }
        const play = encoder.encode(progress.active_play_id);
        const placement = encoder.encode(progress.placement_id);
        const bytes = new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), play.length + placement.length + output.length);
        bytes.set(play);
        bytes.set(placement, play.length);
        bytes.set(output, play.length + placement.length);
        if (api.conduit_browser_form_complete_effect(play.length, placement.length, progress.request_sequence ?? progress.observation_sequence, output.length) < 0) {
          throw new Error(`effect completion refused: ${JSON.stringify(read())}`);
        }
        progress = read();
      }
      if (!effect) throw new Error("bounded browser drive did not reach a manifestation");
      if (api.conduit_browser_form_cancel() < 0) throw new Error(`Form cancellation refused: ${JSON.stringify(read())}`);
      return { effect, receipt: read(), playId };
      }, { source, slug, expected });

      expect(evidence.effect).toMatchObject({
        effect_kind: "manifestation",
        presentation_kind: expected.presentationKind,
      });
      if (expected.text) expect(evidence.effect.text).toBe(expected.text);
      if (expected.morseSegments) {
        expect(evidence.effect.text).toBeNull();
        expect(evidence.effect.segments.length).toBeGreaterThan(0);
      }
      expect(evidence.receipt).toMatchObject({
        disposition: "cancelled",
        active_play_id: evidence.effect.active_play_id,
      });
      console.log(`CONDUIT_FORM_EVIDENCE=${JSON.stringify({
        slug,
        status: "passed",
        plan_id: evidence.effect.plan_id,
        play_id: evidence.playId,
      })}`);
    } catch (error) {
      const reason = String(error?.message ?? error).slice(0, 2_000);
      const refused = reason.includes("Form start refused:") || reason.includes("source interaction refused:");
      console.log(`CONDUIT_FORM_EVIDENCE=${JSON.stringify({
        slug,
        status: refused ? "refused" : "failed",
        reason,
      })}`);
      throw error;
    }
  });
}
