// Report-only browser execution observation; not a native/WASM speed comparison.
import { expect, test } from '../browser/node_modules/@playwright/test/index.mjs';
import { readFile, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import path from 'node:path';
const fixture = new URL('../browser/fixtures/native-value-window.conduit', import.meta.url);

test('finite checked value computation retains real browser timing and memory boundaries', async ({ page }, testInfo) => {
  test.setTimeout(60_000);
  const source = await readFile(fixture, 'utf8');
  if (!process.env.CONDUIT_BROWSER_RUNTIME_WASM) throw new Error('select a sealed runtime with CONDUIT_BROWSER_RUNTIME_WASM');
  const manifest = JSON.parse(await readFile(path.join(path.dirname(path.resolve(process.env.CONDUIT_BROWSER_RUNTIME_WASM)), 'browser-page.json'), 'utf8'));
  await page.goto('/proof/browser/native-value-types.test.html');
  const evidence = await page.evaluate(async source => {
    const fetchStarted = performance.now();
    const response = await fetch('/target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm');
    if (!response.ok) throw new Error(`runtime unavailable: ${response.status}`);
    const wasm = await response.arrayBuffer();
    const fetchMs = performance.now() - fetchStarted;
    const instantiateStarted = performance.now();
    const { instance } = await WebAssembly.instantiate(wasm, {});
    const instantiateMs = performance.now() - instantiateStarted;
    const api = instance.exports;
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    const read = () => JSON.parse(decoder.decode(new Uint8Array(api.memory.buffer,
      api.conduit_browser_plot_output_ptr(), api.conduit_browser_plot_output_len())));
    let inputBytes = 0;
    const write = (...parts) => {
      const total = parts.reduce((sum, part) => sum + part.length, 0);
      if (total > api.conduit_browser_plot_input_capacity()) throw new Error('ABI input extent');
      const input = new Uint8Array(api.memory.buffer, api.conduit_browser_plot_input_ptr(), total);
      let offset = 0;
      for (const part of parts) { input.set(part, offset); offset += part.length; }
      inputBytes += total;
    };
    const rows = [];
    for (let repetition = 0; repetition < 3; repetition += 1) {
      const bytes = encoder.encode(source);
      const host = encoder.encode('browser/overhead-observation');
      const boot = encoder.encode('boot/browser-overhead-observation');
      const before = api.memory.buffer.byteLength;
      const inputBefore = inputBytes;
      const started = performance.now();
      write(bytes);
      if (api.conduit_browser_plot_admit_source_interaction(bytes.length, BigInt(repetition + 1)) < 0) throw new Error(`source admission: ${JSON.stringify(read())}`);
      write(host, boot, bytes);
      if (api.conduit_browser_plot_start(host.length, boot.length, bytes.length, BigInt(repetition + 1)) < 0) throw new Error(`start refused: ${JSON.stringify(read())}`);
      const effect = read();
      const firstEffectMs = performance.now() - started;
      if (effect.effect_kind !== 'manifestation' || effect.face_kind !== 'face/bool-value' || effect.text !== 'true') throw new Error(`wrong computation: ${JSON.stringify(effect)}`);
      const play = encoder.encode(effect.active_play_id);
      const placement = encoder.encode(effect.placement_id);
      write(play, placement);
      if (api.conduit_browser_plot_complete_effect(play.length, placement.length, effect.observation_sequence, 0) < 0) throw new Error(`ack refused: ${JSON.stringify(read())}`);
      const terminal = read();
      const throughTerminalMs = performance.now() - started;
      if (terminal.disposition !== 'completed' || terminal.active_play_id !== effect.active_play_id) throw new Error(`foreign terminal: ${JSON.stringify(terminal)}`);
      rows.push({ repetition, admission_through_first_effect_ms: firstEffectMs,
        admission_through_terminal_ms: throughTerminalMs,
        linear_memory_before_bytes: before, linear_memory_after_bytes: api.memory.buffer.byteLength,
        explicit_js_to_abi_write_bytes: inputBytes - inputBefore,
        effect, terminal });
    }
    const hash = await crypto.subtle.digest('SHA-256', wasm);
    return { browser: navigator.userAgent, wasm_bytes: wasm.byteLength,
      wasm_sha256: [...new Uint8Array(hash)].map(b => b.toString(16).padStart(2, '0')).join(''),
      fetch_ms: fetchMs, compile_and_instantiate_ms: instantiateMs, rows,
      scope: 'actual checked structured-value Plot/Plan/browser kernel; performance.now wall time and linear-memory capacity; not allocator calls, copy totals, CPU time, physical execution or the native arithmetic fixture',
      limits: 'fresh browser process, uncontrolled engine compilation cache; sequential plays in one instance; no threshold, confidence interval or native-speed comparison' };
  }, source);
  expect(evidence.rows).toHaveLength(3);
  const sealed = manifest.files.find(file => file.path === 'runtime.wasm');
  expect(evidence.wasm_sha256).toBe(sealed.sha256.replace(/^sha256:/, ''));
  expect(evidence.wasm_bytes).toBe(sealed.bytes);
  evidence.runtime_source = manifest.source_identity;
  evidence.harness_source = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  evidence.fixture_sha256 = createHash('sha256').update(source).digest('hex');
  const file = testInfo.outputPath('browser-overhead-observation.json');
  await writeFile(file, JSON.stringify(evidence, null, 2));
  await testInfo.attach('browser-overhead-observation.json', { path: file, contentType: 'application/json' });
});
