import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { writeThreeHostWalkthrough } from './three-host-walkthrough.mjs';

async function fixture(run) {
  const root = await mkdtemp(path.join(tmpdir(), 'conduit-walkthrough-'));
  const output = path.join(root, 'output');
  const handbook = path.join(root, 'handbook');
  await Promise.all([mkdir(output), mkdir(handbook)]);
  await Promise.all([
    writeFile(path.join(handbook, 'index.html'), '<header class="site-header"><a data-section="handbook" aria-current="page" href="/conduit/handbook/">Handbook</a><a data-section="journeys" href="/conduit/journeys/">Journeys</a></header>'),
    writeFile(path.join(handbook, 'conduit.css'), ''),
    writeFile(path.join(handbook, 'chrome.css'), ''),
  ]);
  try { await run(output, handbook); } finally { await rm(root, { recursive: true, force: true }); }
}

const partial = {
  schema: 'conduit.body/three-host-owner-journey@1',
  proof_class: 'live-local-installed-owner-qmp-pinned-chromium-direct-speech',
  run_id: 'run', body_id: '<unsafe-body>', native_source_commit: 'a'.repeat(40),
};
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

test('partial report renders eight navigable chapters and explicit gaps', () => fixture(async (output, handbook) => {
  const result = await writeThreeHostWalkthrough(output, handbook, partial);
  const html = await readFile(path.join(output, result.path), 'utf8');
  assert.equal((html.match(/<article id=/g) ?? []).length, 8);
  assert.equal((html.match(/Not yet captured in this run/g) ?? []).length, 8);
  assert.match(html, /0 of 8 chapters complete/);
  assert.match(html, /&lt;unsafe-body&gt;/);
  assert.doesNotMatch(html, /<unsafe-body>/);
  assert.match(html, /id="loss"/);
  assert.match(html, /href="#return"/);
  assert.match(html, /aria-current="page"/);
  assert.doesNotMatch(html, /<audio /);
}));

test('same-Play audio claim requires an exact retained WAV', () => fixture(async (output, handbook) => {
  await assert.rejects(writeThreeHostWalkthrough(output, handbook, {
    ...partial,
    screen_free_audio: {
      proof_class: 'selected-screen-free-same-play-listener-audio-subset',
      source_commit: partial.native_source_commit, human_hearing_observed: false,
      public_selected_clip_count: 1,
      selected: [{ path: 'screen-free-audio/birth-review.wav', wav_sha256: '0'.repeat(64),
        bytes: 44, plan_id: 'plan', play_id: 'play', source_show_id: 'show', speaker_frames_committed: 1 }],
    },
  }), /ENOENT/);
}));

test('selected Birth audio displays exact escaped words and withholds unlabeled clips', () => fixture(async (output, handbook) => {
  const artifact = async (name, contents) => {
    const bytes = Buffer.from(contents);
    const file = path.join(output, name);
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, bytes);
    return { path: name, sha256: digest(bytes), bytes: bytes.length };
  };
  const zero = await artifact('zero-body-before.json', '{}');
  const input = await artifact('birth-input.txt', 'choose clock\nbirth\n');
  const transcript = await artifact('birth-transcript.txt', 'Body born.');
  const wav = await artifact('screen-free-audio/birth-review.wav', 'RIFFtestWAVE');
  const words = 'Review <clock> & birth.';
  const report = {
    ...partial,
    birth: { proof_class: 'installed-screen-free-birth-selected-alsa',
      body_id: partial.body_id, source_commit: partial.native_source_commit,
      zero_body_observed: true, confirmation_observed: true,
      zero_body_receipt: zero, input, transcript },
    screen_free_audio: { proof_class: 'selected-screen-free-same-play-listener-audio-subset',
      source_commit: partial.native_source_commit, human_hearing_observed: false,
      public_selected_clip_count: 1,
      selected: [{ moment: 'birth-review', excerpt: 'opening', ...wav,
        wav_sha256: wav.sha256, face_revision: '1', plan_id: 'plan', play_id: 'play',
        source_show_id: 'show', speaker_frames_committed: 1,
        spoken_words: words, spoken_words_html: 'Review &lt;clock&gt; &amp; birth.',
        spoken_segment_sha256: [digest(Buffer.from(words))] }],
    },
  };
  await writeThreeHostWalkthrough(output, handbook, report);
  let html = await readFile(path.join(output, 'walkthrough.html'), 'utf8');
  assert.match(html, /Review &lt;clock&gt; &amp; birth\./);
  assert.doesNotMatch(html, /Review <clock> & birth\./);
  assert.match(html, /<audio controls/);
  assert.match(html, /1 of 8 chapters complete/);

  delete report.screen_free_audio.selected[0].spoken_words;
  delete report.screen_free_audio.selected[0].spoken_words_html;
  delete report.screen_free_audio.selected[0].spoken_segment_sha256;
  await writeThreeHostWalkthrough(output, handbook, report);
  html = await readFile(path.join(output, 'walkthrough.html'), 'utf8');
  assert.match(html, /Selected Birth WAVs lack their exact producer-verified spoken words/);
  assert.doesNotMatch(html, /<audio controls/);
  assert.match(html, /0 of 8 chapters complete/);
}));
