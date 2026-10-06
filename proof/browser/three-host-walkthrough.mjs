// A private, human-readable view of one completed live proof receipt.
// The complete eight-chapter public journey has a separate publication gate.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { copyFile, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const escape = value => String(value).replace(/[&<>"']/g, character => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
})[character]);
const figure = (file, alt, caption) =>
  `<figure><img loading="lazy" src="${file}" alt="${escape(alt)}"><figcaption>${escape(caption)}</figcaption></figure>`;
const chapter = (id, number, title, explanation, media) =>
  `<article id="${id}" aria-labelledby="${id}-title"><p class="step">${number} · Real action</p><h2 id="${id}-title">${title}</h2><p>${explanation}</p>${media}</article>`;

export async function writeThreeHostWalkthrough(output, handbook, report) {
  const packageHtml = await readFile(path.join(handbook, 'index.html'), 'utf8');
  const masthead = packageHtml.match(/<header class="site-header">[\s\S]*?<\/header>/)?.[0];
  assert.ok(masthead, 'exact-source Handbook has no shared site navigation');
  const navigation = masthead
    .replace('data-section="handbook" aria-current="page"', 'data-section="handbook"')
    .replace('data-section="journeys"', 'data-section="journeys" aria-current="page"')
    .replaceAll('href="/conduit/', 'href="https://dancxjo.github.io/conduit/');
  await Promise.all(['conduit.css', 'chrome.css'].map(name =>
    copyFile(path.join(handbook, name), path.join(output, name))));

  const sections = [];
  if (report.birth) {
    const birth = report.birth;
    assert.equal(birth.body_id, report.body_id);
    assert.equal(birth.owner_host_id, report.owner_host_id);
    assert.equal(birth.owner_boot_id, report.owner_boot_id);
    assert.equal(birth.source_commit, report.native_source_commit);
    assert.equal(birth.zero_body_receipt.path, '../zero-body-before.json');
    const transcript = await readFile(path.join(output, '..', 'birth-transcript.txt'), 'utf8');
    sections.push(chapter('birth', 1, 'Make the Body without a screen',
      'Start the freshly installed Linux Host with no Body. Its screen-free Crèche reads its available controls. Name the Body, select the reviewed clock Plot, review the current choices, and explicitly activate Birth. The installed Host retains the resulting Body on this same Boot.' +
      (birth.speaker_playback_selected
        ? ' An explicitly selected ALSA speaker drained the produced reading. The current playback path uses a second StdHost sharing owner Host and Boot identities, so this run does not establish owner-instance speech realization or human listening.'
        : ' This run used text readout; it does not claim speech audio or speaker playback.'),
      '<details><summary>Read the actual nonvisual Birth session</summary><pre>'
      + escape(transcript) + '</pre></details><p><a href="../zero-body-before.json">Inspect the live zero-Body Host and Boot before Birth</a></p>'));
  }
  const offset = report.birth ? 1 : 0;
  sections.push(...[
    chapter('browser', 1 + offset, 'Open the clock in a browser',
      'The browser joins the Linux owner’s existing Body. It sees the current clock through its own graphical Mask, with a fresh Host and Boot identity.',
      figure('browser-before.png', 'Browser Face before the clock changes', 'The browser’s first acknowledged Face.')),
    chapter('native', 2 + offset, 'Meet the same Body on ConduitOS',
      'Boot the provisioned ConduitOS image in QEMU. Its Part joins this Body; the native screen receives the owner’s current Face and acknowledges its Show.',
      figure('native/owner-before.png', 'ConduitOS native Mask before its action', 'The QMP capture comes from the running guest.')),
    chapter('native-action', 3 + offset, 'Change the clock from ConduitOS',
      'Use the guest’s keyboard control to ask for 500 milliseconds. The owner accepts the typed action. Refresh the browser to see the same changed Face.',
      figure('native/owner-after.png', 'ConduitOS native Mask after changing the clock to 500 milliseconds', 'The guest shows the owner’s refreshed Face.')
      + figure('browser-after-native.png', 'Browser Face showing the guest’s 500 millisecond change', 'A second Host sees the change without a second Body birth.')),
    chapter('browser-action', 4 + offset, 'Change it back in the browser',
      'Use the browser’s control to request 1000 milliseconds. The owner accepts it and the browser acknowledges a new Show.',
      figure('browser-after-browser.png', 'Browser Face after changing the clock to 1000 milliseconds', 'The browser action changed the shared owner state.')),
    chapter('terminal-action', 5 + offset, 'Read and change it in a terminal',
      'Read the current Face, focus its available control, type 500, and apply. The terminal Mask sends a typed semantic action and receives the new Face. The browser can read that result too.',
      figure('browser-after-terminal.png', 'Browser Face after the terminal changed the clock to 500 milliseconds', 'The browser confirms the terminal action on the same Body.')
      + '<details><summary>Read the actual terminal session</summary><pre>'
      + escape(await readFile(path.join(output, 'terminal-face.txt'), 'utf8'))
      + '</pre></details>'),
  ]);
  if (report.direct_speech) {
    const clips = [];
    const produced = [];
    for (const [index, wav] of report.direct_speech.wavs.entries()) {
      const receipt = JSON.parse(await readFile(path.join(output, 'speech-direct',
        `direct-batch-${index + 1}-receipt.json`)));
      const readable = receipt.source_segments.map(segment => segment.text).join(' ');
      produced.push({ wav, readable });
      clips.push(`<li><p>${escape(readable)}</p><audio controls preload="none" src="${wav.path}"><a href="${wav.path}">Download speech batch ${index + 1}</a></audio></li>`);
    }
    const featured = produced.find(item =>
      item.readable.includes('The current clock interval is 500 milliseconds'))
      ?? produced[0];
    sections.push(chapter('direct-speech', 6 + offset, 'Hear the current Face',
      `Request a mechanical full-Face reading while the three Hosts remain live. The runtime committed ${report.direct_speech.batch_count} ordered speech batches and produced real PCM WAV files. This capture does not claim speaker playback or human listening.`,
      `<figure class="audio-feature"><figcaption><strong>Listen to the current clock</strong><p>${escape(featured.readable)}</p></figcaption><audio controls preload="metadata" src="${featured.wav.path}"><a href="${featured.wav.path}">Download this produced speech clip</a></audio></figure><details><summary>Hear the complete ${clips.length}-batch Face reading</summary><ol class="audio-list">${clips.join('')}</ol></details>`));
  }
  if (report.llm_speech) {
    const original = JSON.parse(await readFile(path.join(output, 'speech-llm',
      'original-model-output.json'))).output;
    const routeLoss = await readFile(path.join(output, 'model-route-loss.txt'), 'utf8');
    sections.push(chapter('llm-speech', 7 + offset, 'Ask for a grounded explanation',
      'Ask the local model to explain the current Face while the three Hosts remain live. The finite Presenter validates its original wording against that Face before an ordinary spoken Mask produces this WAV. The local spoken Show belongs to the proof Host, not an owner-sealed wardrobe route. Then withdraw only this capture’s loopback model route: the next request refuses without an audio file; the underlying Ollama service stays running.',
      `<figure class="audio-feature"><figcaption><strong>Listen to the validated explanation</strong><p>${escape(report.llm_speech.validated_text)}</p></figcaption><audio controls preload="metadata" src="${report.llm_speech.wav.path}"><a href="${report.llm_speech.wav.path}">Download the produced explanation</a></audio></figure>`
      + `<details><summary>Compare the original model output and validated speech</summary><h3>Original model output</h3><pre>${escape(original)}</pre><h3>Validated spoken text</h3><p>${escape(report.llm_speech.validated_text)}</p><p><a href="speech-llm/validation.json">Inspect validation receipt</a> · <a href="speech-llm/model-validation.json">Inspect model validation</a></p></details>`
      + `<details><summary>Inspect the configured route withdrawal</summary><p>The producer closed its own forwarding endpoint. This is a configured route refusal, not an Ollama daemon shutdown or wardrobe replacement.</p><pre>${escape(routeLoss)}</pre></details>`));
  }
  if (report.screen_free_lull) {
    const lull = report.screen_free_lull;
    assert.equal(lull.body_id, report.body_id);
    assert.equal(lull.run_id, report.run_id);
    assert.equal(lull.source_commit, report.native_source_commit);
    const input = await readFile(path.join(output, '..', 'lull-input.txt'));
    assert.equal(input.length, lull.input.bytes);
    assert.equal(digest(input), lull.input.sha256);
    const transcriptBytes = await readFile(path.join(output, '..', 'lull-transcript.txt'));
    assert.equal(transcriptBytes.length, lull.transcript.bytes);
    const transcript = transcriptBytes.toString('utf8');
    assert.equal(digest(transcript), lull.transcript.sha256);
    sections.push(chapter('lull', sections.length + 1, 'Leave the Body well, without a screen',
      'Reenter the installed owner’s retained Body through its nonvisual interface. Read the whole current Face, focus its exact Lull action, and activate it. The owner acknowledges the action, stops the current Play, and offers Wake on a new Face while retaining the same Body and Host Boot.' +
      (lull.speaker_playback_selected
        ? ' The selected speaker drained the produced reading; human hearing was not observed.'
        : ' This run used text readout and does not claim audio playback.'),
      `<details><summary>Read the actual nonvisual Lull session</summary><pre>${escape(transcript)}</pre></details>`
      + `<p><a href="../lull-input.txt">Inspect the submitted commands</a> · <a href="report.json">Inspect the exact Face, Show, action, and result identities</a></p>`));
  }
  const ids = [...(report.birth ? ['birth'] : []), 'browser', 'native', 'native-action', 'browser-action', 'terminal-action'];
  if (report.direct_speech) ids.push('direct-speech');
  if (report.llm_speech) ids.push('llm-speech');
  if (report.screen_free_lull) ids.push('lull');
  const titles = [...(report.birth ? ['Birth'] : []), 'Browser', 'ConduitOS', 'Native action', 'Browser action', 'Terminal action'];
  if (report.direct_speech) titles.push('Direct speech');
  if (report.llm_speech) titles.push('Model explanation');
  if (report.screen_free_lull) titles.push('Lull');
  const stepLinks = ids.map((id, index) => `<li><a href="#${id}">${index + 1}. ${titles[index]}</a></li>`).join('');
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>One clock, three live Hosts · Conduit development proof</title><link rel="stylesheet" href="conduit.css"><link rel="stylesheet" href="chrome.css"><style>
body{margin:0;background:var(--conduit-background);color:var(--conduit-text-primary)}.proof{max-width:74rem;margin:auto;padding:2rem 1.25rem 5rem}.proof h1,.proof h2{font-family:var(--conduit-font-editorial);line-height:1.15}.proof h1{font-size:clamp(2.5rem,6vw,5rem);max-width:14ch}.proof h2{font-size:clamp(1.8rem,3vw,2.8rem)}.proof p{max-width:68ch}.proof .lede{font-size:1.25rem;color:var(--conduit-text-secondary)}.proof .boundary{border-left:.25rem solid var(--conduit-emphasis);padding:1rem;background:var(--conduit-reading-paper)}.proof nav ol{display:flex;flex-wrap:wrap;gap:.75rem 1.5rem;padding:0;list-style:none}.proof a{color:var(--conduit-structure-primary)}.proof a:focus-visible,.proof summary:focus-visible{outline:3px solid var(--conduit-focus);outline-offset:3px}.proof article{padding:3rem 0;border-top:1px solid var(--conduit-structure-secondary)}.proof .step{color:var(--conduit-emphasis);font-weight:700;letter-spacing:.06em;text-transform:uppercase}.proof figure{margin:1.5rem 0 2.5rem}.proof img{display:block;width:100%;height:auto;border:1px solid var(--conduit-structure-secondary);border-radius:var(--conduit-radius-panel)}.proof figcaption{padding:.6rem 0;color:var(--conduit-text-secondary)}.proof details,.proof .audio-feature{padding:1rem;background:var(--conduit-reading-paper);border:1px solid var(--conduit-structure-secondary);border-radius:var(--conduit-radius-panel)}.proof pre{max-height:30rem;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere}.proof .audio-list{padding-left:1.5rem}.proof .audio-list li{padding:1rem 0;border-top:1px solid var(--conduit-structure-secondary)}.proof audio{width:min(100%,40rem)}
</style></head><body data-application-theme="conduit.presentation/phosphor@1"><a class="conduit-skip-link" href="#proof">Skip to the proof</a>${navigation}<main id="proof" class="proof"><header><p class="step">Development proof · captured from one run</p><h1>One clock, three live Hosts</h1><p class="lede">${report.birth ? 'Birth a shared clock through nonvisual controls, then change it' : 'Change a shared clock'} from ConduitOS, a browser, and a terminal. See each result where you meet the same Body; hear its current Face through direct${report.llm_speech ? ' and validated model-assisted' : ''} speech.</p><p class="boundary">This is a live local proof, not the complete eight-chapter public journey. QMP establishes emulator execution. Produced audio is distinct from speaker playback and human listening.</p></header><nav aria-label="Proof steps"><ol>${stepLinks}</ol></nav>${sections.join('')}<details><summary>Inspect source and evidence</summary><p>Body <code>${escape(report.body_id)}</code> · source <code>${escape(report.native_source_commit)}</code> · run <code>${escape(report.run_id)}</code>.</p><p><a href="report.json">Digest-bound proof report</a> · <a href="native/owner-action-proof.json">Native QMP receipt</a>${report.birth ? ' · <a href="../birth-input.txt">Screen-free input</a> · <a href="../birth-transcript.txt">Birth transcript</a>' : ''}${report.direct_speech ? ' · <a href="speech-direct/manifest.json">Direct speech manifest</a>' : ''}${report.llm_speech ? ' · <a href="speech-llm/manifest.json">Model-assisted speech manifest</a>' : ''} · <a href="https://github.com/dancxjo/conduit/wiki">Project wiki</a>.</p><p>The owner retains the workload truth. This capture does not prove wardrobe preference, host loss and recovery, owner-sealed spoken Mask selection, or release publication.</p></details></main></body></html>`;
  const file = path.join(output, 'walkthrough.html');
  await writeFile(file, html);
  return { path: 'walkthrough.html', bytes: Buffer.byteLength(html) };
}
