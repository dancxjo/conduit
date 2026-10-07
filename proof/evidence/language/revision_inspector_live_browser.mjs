// Read-only contemporaneous observation. This never writes the producer's file.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import assert from 'node:assert/strict';
const [url, tracePath, output] = process.argv.slice(2);
if (!url || !tracePath || !output) throw Error('Usage: node revision_inspector_live_browser.mjs URL actual.jsonl output-directory');
mkdirSync(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const started = new Date().toISOString();
try {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(url);
  await page.waitForFunction(() => window.liveObservation);
  const initial = await page.evaluate(() => window.liveObservation);
  assert.equal(initial.producer_state, 'running');
  assert.deepEqual(errors, []);
  writeFileSync(join(output, 'observer-start.json'), JSON.stringify({ started, url, initial, producer_file_mutated: false }, null, 2) + '\n');
  console.log(JSON.stringify({ event: 'observer_attached', started, initial }));
  await page.waitForFunction(after => evidence.events.some((event, index) =>
    index >= after && event.event === 'snapshot' &&
    (typeof (event.receipt ?? event).beam_bytes === 'string' || Array.isArray((event.receipt ?? event).beam_bytes)) && (event.receipt ?? event).model_invocations > 0
  ), initial.count, { timeout: 60 * 60 * 1000 });
  const observed = await page.evaluate(after => {
    const index = evidence.events.findIndex((event, index) => index >= after && event.event === 'snapshot' && (typeof (event.receipt ?? event).beam_bytes === 'string' || Array.isArray((event.receipt ?? event).beam_bytes)) && (event.receipt ?? event).model_invocations > 0);
    document.getElementById('event').value = index;
    document.getElementById('event').dispatchEvent(new Event('input'));
    return { index, event: evidence.events[index], observation: window.liveObservation };
  }, initial.count);
  const raw = readFileSync(tracePath, 'utf8');
  const complete = raw.slice(0, raw.lastIndexOf('\n') + 1);
  const actual = complete.split('\n').filter(line => line.trim()).map(JSON.parse);
  assert.deepEqual(observed.event, actual[observed.index]);
  const displayed = JSON.parse(await page.locator('#raw').textContent());
  const receipt = observed.event.receipt ?? observed.event;
  assert.deepEqual(observed.event.receipt ? displayed.receipt : displayed, receipt);
  assert(receipt.candidates.length > 0);
  if (Array.isArray(receipt.beam_bytes)) assert(receipt.beam_bytes.length > 0 && receipt.beam_bytes.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255));
  assert.deepEqual(errors, []);
  await page.screenshot({ path: join(output, 'actual-native-snapshot.png'), fullPage: true });
  const result = {
    started, observed_at: new Date().toISOString(), initial,
    observation: observed.observation, snapshot_sequence: observed.event.sequence,
    actual_native_snapshot_observed_after_attach: true,
    displayed_receipt_matches_actual_producer_file: true,
    producer_file_mutated: false,
    parser_started_by_observer: false,
    revision_chain_proved: false,
    played_audio_proved: false,
  };
  writeFileSync(join(output, 'live-native-observation.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result));
} finally {
  await browser.close();
}
