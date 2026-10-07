import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const script = new URL('./retain-three-host-development.mjs', import.meta.url).pathname;
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const chapters = ['birth', 'join', 'start', 'see', 'hear', 'loss', 'return', 'lull'];

async function fixture(run) {
  const root = await mkdtemp(path.join(tmpdir(), 'conduit-retain-three-host-'));
  const source = path.join(root, 'private', 'three-host');
  const output = path.join(root, 'public');
  await mkdir(path.join(source, 'native'), { recursive: true });
  const words = 'Review <clock> & birth.';
  const wav = Buffer.concat([Buffer.from('RIFF'), Buffer.alloc(4), Buffer.from('WAVE'), Buffer.alloc(44)]);
  const file = path.join(source, 'screen-free-audio/birth-review.wav');
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, wav);
  await writeFile(path.join(source, 'native/guest.serial.log'), 'guest ready\n');
  await writeFile(path.join(source, 'native/qmp.jsonl'), '{}\n');
  const html = `<!doctype html><html><head><style>.proof{}</style><link rel="stylesheet" href="chrome.css"></head><body><p>Development capture · 8 of 8 chapters complete</p><p>The public Journey and accepted release require separate gates</p>${chapters.map(id => `<article id="${id}"></article>`).join('')}<audio src="screen-free-audio/birth-review.wav"></audio><a href="report.json">report</a></body></html>`;
  await writeFile(path.join(source, 'walkthrough.html'), html);
  await writeFile(path.join(source, 'chrome.css'), 'body{}');
  const report = {
    schema: 'conduit.body/three-host-owner-journey@1',
    proof_class: 'live-local-installed-owner-qmp-pinned-chromium-selected-model-speaker',
    native_source_commit: 'a'.repeat(40), run_id: 'run-1', body_id: 'body',
    owner_host_id: 'owner', guest_host_id: 'guest', browser_host_id: 'browser',
    walkthrough: { path: 'walkthrough.html', bytes: Buffer.byteLength(html), sha256: sha(Buffer.from(html)),
      assets: [{ path: 'chrome.css', sha256: sha(Buffer.from('body{}')) }] },
    screen_free_audio: {
      proof_class: 'selected-screen-free-same-play-listener-audio-subset',
      source_commit: 'a'.repeat(40), human_hearing_observed: false,
      selected: [{ moment: 'birth-review', path: 'screen-free-audio/birth-review.wav',
        wav_sha256: sha(wav), bytes: wav.length, spoken_words: words,
        spoken_words_html: 'Review &lt;clock&gt; &amp; birth.',
        spoken_segment_sha256: [sha(Buffer.from(words))], plan_id: 'plan', play_id: 'play',
        source_show_id: 'show', speaker_frames_committed: 1 }],
    },
  };
  const save = () => writeFile(path.join(source, 'report.json'), `${JSON.stringify(report)}\n`);
  await save();
  try { await run({ root, source, output, report, save }); }
  finally { await rm(root, { recursive: true, force: true }); }
}
const retain = (root, output) => spawnSync(process.execPath, [script, path.join(root, 'private'), output], { encoding: 'utf8' });

test('retains eight-chapter page and exact selected original WAV as diagnostic evidence', () => fixture(async ({ root, output }) => {
  const result = retain(root, output);
  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(await readFile(path.join(output, 'manifest.json'), 'utf8'));
  assert.equal(manifest.result, 'diagnostic-incomplete');
  assert.equal(manifest.git_commit, 'a'.repeat(40));
  assert.ok(manifest.outputs.some(item => item.path === 'screen-free-audio/birth-review.wav' && item.kind === 'audio'));
  assert.match(await readFile(path.join(output, 'index.html'), 'utf8'), /8 of 8 chapters complete/);
}));

test('retains an explicit gap without publishing an unlabeled selected WAV', () => fixture(async ({ root, source, output, report, save }) => {
  delete report.screen_free_audio.selected[0].spoken_words;
  delete report.screen_free_audio.selected[0].spoken_words_html;
  delete report.screen_free_audio.selected[0].spoken_segment_sha256;
  const oldHtml = await readFile(path.join(source, 'walkthrough.html'), 'utf8');
  const html = oldHtml.replace('Development capture · 8 of 8 chapters complete',
    'Development capture · 7 of 8 chapters complete')
    .replace('<audio src="screen-free-audio/birth-review.wav"></audio>',
      '<p>Not yet captured in this run.</p>');
  await writeFile(path.join(source, 'walkthrough.html'), html);
  report.walkthrough.bytes = Buffer.byteLength(html);
  report.walkthrough.sha256 = sha(Buffer.from(html));
  await save();
  const result = retain(root, output);
  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(await readFile(path.join(output, 'manifest.json'), 'utf8'));
  assert.ok(!manifest.outputs.some(item => item.path === 'screen-free-audio/birth-review.wav'));
  assert.match(await readFile(path.join(output, 'index.html'), 'utf8'), /Not yet captured in this run/);
}));

test('refuses private material in a linked report', () => fixture(async ({ root, output, report, save }) => {
  report.secret = 'do not publish';
  await save();
  const result = retain(root, output);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /private material appears in report.json/);
}));

test('refuses a changed original selected WAV', () => fixture(async ({ root, source, output }) => {
  await writeFile(path.join(source, 'screen-free-audio/birth-review.wav'), 'RIFFchangedWAVE');
  const result = retain(root, output);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /producer digest changed: screen-free-audio\/birth-review.wav/);
}));
