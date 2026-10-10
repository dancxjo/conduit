import { expect, test } from "@playwright/test";
import { writeFile } from "node:fs/promises";

const prefixes = [
  ["da", 1], ["h", 2], ["k", 3], ["M", 6], ["G", 9], ["T", 12],
  ["P", 15], ["E", 18], ["Z", 21], ["Y", 24], ["R", 27], ["Q", 30],
  ["d", -1], ["c", -2], ["m", -3], ["µ", -6], ["n", -9], ["p", -12],
  ["f", -15], ["a", -18], ["z", -21], ["y", -24], ["r", -27], ["q", -30],
];
function converted(coefficient, exponent) {
  return `variant/is(.result, "converted") ? (.result.converted.coefficient == ${coefficient} ? .result.converted.exponent == ${exponent} : false) : false`;
}
function refused(reason) {
  return `variant/is(.result, "refused") ? .result.refused == "${reason}" : false`;
}
function source(kind, left, right, predicate) {
  const names = kind.includes("compare") ? ["left", "right"] : ["source", "to"];
  return `plot checked-quantity {
 operation: ${kind}(${names[0]} = ${left}, ${names[1]} = ${right})
 show: presentation/bool-value
 operation.receipt >> (${predicate}) >> show.value
}.`;
}
const convertedCases = [
  ...prefixes.map(([prefix, exponent]) => ["units/convert", `1${prefix}m`, "m", converted(1, exponent)]),
  ["units/convert", "1kHz", "Hz", converted(1, 3)],
  ["units/convert", "1µs", "ns", converted(1, 3)],
  ["units/convert", "1cm²", "mm²", converted(1, 2)],
  ["units/convert", "0°C", "K", converted(27315, -2)],
  ["units/convert", "30°C", "°F", converted(86, 0)],
  ["units/convert", "1Qm", "qm", converted(1, 60)],
  ["units/convert", "1qm", "Qm", converted(1, -60)],
  ["units/convert", "1MB", "B", converted(1, 6)],
  ["units/convert", "1MiB", "B", converted(1048576, 0)],
];
const roleCases = [
  ["units/convert-temperature-difference", "9°F", "K", converted(5, 0)],
  ["units/convert-temperature-difference", "1m°C", "K", converted(1, -3)],
  ["units/compare", "1000mm", "0.001km", 'variant/is(.result, "equal")'],
  ["units/compare", "1°F", "0°C", 'variant/is(.result, "less")'],
  ["units/compare", "1MB", "1MiB", 'variant/is(.result, "less")'],
  ["units/compare-temperature-differences", "9°F", "5K", 'variant/is(.result, "equal")'],
  ["units/convert", "1°F", "°C", refused("inexact")],
  ["units/convert", "1Hz", "m", refused("incompatible-dimensions")],
  ["units/convert", "1Qm³", "qm³", refused("overflow")],
  ["units/compare", "1m", "1s", refused("incompatible-dimensions")],
];

const comparatorCases = ["units/converted-equals", "=?"].flatMap((comparator) => [
  ["1kHz", "Hz", "1000Hz", "Exactly 1000 Hz"],
  ["1kHz", "Hz", "999Hz", "Conversion did not yield exactly 1000 Hz"],
  ["1Hz", "m", "1m", "Conversion did not yield exactly 1000 Hz"],
  ["1kHz", "Hz", "1m", "Conversion did not yield exactly 1000 Hz"],
].map(([left, right, expected, text]) => ({
  kind: "units/converted-equals", left, right, expected, invocation: comparator,
  expected_text: text, expected_presentation: "presentation/text", implementation: "browser/converted-equals@1",
  source: `${comparator === "=?" ? "with units/converted-equals as =?\n" : ""}plot convert-pitch-demo {
 operation: units/convert(source = ${left}, to = ${right})
 exact: ${comparator}(expected = ${expected})
 show: presentation/text
 operation.receipt >> exact.receipt
 exact.result >> (. ? "Exactly 1000 Hz" : "Conversion did not yield exactly 1000 Hz") >> show.text
}.`,
})));

for (const [name, cases] of [["official prefix and affine corpus", convertedCases], ["semantic roles and retained refusals", roleCases], ["full-name and scoped alias receipt comparator", comparatorCases]]) {
  test(`exact quantities execute ${name} through the browser kernel`, async ({ page }, testInfo) => {
    test.setTimeout(60_000);
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto("/proof/browser/exact-quantity.test.html");
    const sources = cases.map((entry) => Array.isArray(entry) ? { kind: entry[0], left: entry[1], right: entry[2], source: source(...entry) } : entry);
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
        const host = encoder.encode("browser/exact-quantity-proof");
        const boot = encoder.encode("boot/exact-quantity-proof");
        write(bytes);
        if (api.conduit_browser_plot_admit_source_interaction(bytes.length, BigInt(index + 1)) < 0) throw new Error(`source interaction: ${JSON.stringify(read())}`);
        write(host, boot, bytes);
        if (api.conduit_browser_plot_start(host.length, boot.length, bytes.length, BigInt(index + 1)) < 0) throw new Error(`${request.left} -> ${request.right}: ${JSON.stringify(read())}`);
        const effect = read();
        if (effect.effect_kind !== "manifestation" || effect.presentation_kind !== (request.expected_presentation ?? "presentation/bool-value") || effect.text !== (request.expected_text ?? "true")) throw new Error(`${request.left} -> ${request.right}: ${JSON.stringify(effect)}`);
        const planned = effect.expanded_gears.find((gear) => gear.kind_id === request.kind);
        if (!(request.implementation ? planned?.implementation_id === request.implementation : planned?.implementation_id.startsWith("browser/exact-"))) throw new Error("exact operation did not select the installed browser Back");
        document.querySelector("#result").textContent = effect.text;
        const play = encoder.encode(effect.active_play_id);
        const placement = encoder.encode(effect.placement_id);
        write(play, placement);
        if (api.conduit_browser_plot_complete_effect(play.length, placement.length, effect.observation_sequence, 0) < 0) throw new Error(`effect acknowledgment: ${JSON.stringify(read())}`);
        const receipt = read();
        if (receipt.disposition !== "completed" || receipt.active_play_id !== effect.active_play_id) throw new Error(`completion: ${JSON.stringify(receipt)}`);
        rows.push({ kind: request.kind, left: request.left, right: request.right, expected: request.expected, invocation: request.invocation, source: request.source, effect, receipt });
      }
      const digest = await crypto.subtle.digest("SHA-256", wasm);
      return { rows, wasm_sha256: [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join(""), browser: navigator.userAgent };
    }, sources);
    expect(errors).toEqual([]);
    expect(evidence.rows).toHaveLength(cases.length);
    await expect(page.locator("#result")).toHaveText(sources.at(-1).expected_text ?? "true");
    const evidencePath = testInfo.outputPath("exact-quantity-browser-evidence.json");
    await writeFile(evidencePath, JSON.stringify(evidence, null, 2));
    await testInfo.attach("exact-quantity-browser-evidence.json", { path: evidencePath, contentType: "application/json" });
  });
}
