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
      'Boot the provisioned ConduitOS image in QEMU. Its Part joins this Body and first shows the owner’s current Face for reading. In the browser, inspect the owner’s wardrobe, wear the newly admitted Native graphical Mask, doff the browser Mask, and prefer Native graphics. The current browser Show stays valid until the person doffs it; preference alone does not interrupt that Show. Then press F5 on the native screen to request a fresh Show. The owner acknowledges that selected Show before any native action.',
      figure('native/owner-standby.png', 'ConduitOS reading the owner Face while awaiting Mask selection', 'The native screen gives the person the F5 continuation instruction; the owner has not selected this Show yet.')
      + figure('native/owner-before.png', 'ConduitOS native Mask after explicit selection', 'The QMP capture comes from the running guest after a browser wardrobe action and a real F5 key press.')),
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
  if (report.browser_wardrobe) {
    const record = JSON.parse(await readFile(path.join(output, report.browser_wardrobe.path), 'utf8'));
    assert.equal(record.schema, 'conduit.proof/owner-browser-wardrobe@1');
    assert.equal(record.body_id, report.body_id);
    assert.equal(record.run_id, report.run_id);
    assert.equal(record.before.owner_plan_id, record.recovered.owner_plan_id);
    assert.equal(record.recovered.show_id, report.browser_wardrobe.selected_show_id);
    sections.push(chapter('wardrobe', 6 + offset, 'Change the Body’s wardrobe',
      'Inspect the owner’s admitted Mask routes in the browser. Doff the browser Mask: its Show is withdrawn while the Body and immutable presentation Plan remain. Wear it again, explicitly prefer it, and request a fresh Face. The owner selects the already sealed route and acknowledges a new Show. This step exercises owner policy and same-Plan recovery; it does not stand in for a different Host losing its route.',
      figure('browser-wardrobe.png', 'Browser wardrobe after doff, wear, preference, and fresh Show',
        'The browser displays the owner’s current routes, preference, and selected Show.')
      + `<details><summary>Inspect the exact owner transitions</summary><p>Owner Plan <code>${escape(record.before.owner_plan_id)}</code> · browser route <code>${escape(record.browser_route_id)}</code> · revisions ${escape(record.before.wardrobe_revision_decimal)} to ${escape(record.recovered.wardrobe_revision_decimal)}.</p><p><a href="browser-wardrobe.json">Read all five owner reports</a></p></details>`));
  }
  if (report.owner_selected_speech) {
    const speech = report.owner_selected_speech;
    assert.equal(speech.body_id, report.body_id);
    assert.equal(speech.owner_host_id, report.owner_host_id);
    assert.equal(speech.owner_boot_id, report.owner_boot_id);
    assert.ok(speech.source_show_id && speech.browser_route_plan_id);
    assert.equal(speech.wav_artifact_from_this_play, true);
    assert.equal(speech.human_hearing_observed, false);
    const clips = speech.batches.map((batch, index) => {
      assert.equal(batch.wav.source, 'same-selected-speaker-play');
      return `<li><p>${escape(batch.spoken_segments.join(' '))}</p>${batch.wav.audible
        ? `<audio controls preload="none" src="${escape(batch.wav.path)}"><a href="${escape(batch.wav.path)}">Download speaker Play ${index + 1}</a></audio>`
        : '<p>This completed Play delivered digital silence; no audible clip is offered.</p>'}</li>`;
    });
    const audible = speech.batches.filter(batch => batch.wav.audible);
    assert.ok(audible.length > 0, 'the selected speaker chapter needs an audible Play');
    const featured = audible.find(batch => batch.spoken_segments.some(text =>
      text.includes('The current clock interval is 500 milliseconds'))) ?? audible[0];
    sections.push(chapter('owner-selected-speech', sections.length + 1,
      'Ask the owner to read this view aloud',
      `From the joined browser, select “Read this view aloud,” then check its outcome. The installed Linux owner reports ${speech.batches.length} completed speaker Plays through its preselected equipment; ${audible.length} carried non-silent PCM. Each playable WAV records the PCM fanned to the speaker in that same Plan and Play; the words beside it are the ordered committed segments. This establishes the device's completed output path, not attended human hearing.`,
      `<figure class="audio-feature"><figcaption><strong>Listen to this speaker Play</strong><p>${escape(featured.spoken_segments.join(' '))}</p></figcaption><audio controls preload="metadata" src="${escape(featured.wav.path)}"><a href="${escape(featured.wav.path)}">Download this speaker Play</a></audio></figure><details><summary>Inspect every completed speaker Play</summary><ol class="audio-list">${clips.join('')}</ol></details><details><summary>Inspect the exact output</summary><p>Browser route Plan <code>${escape(speech.browser_route_plan_id)}</code> · source Show <code>${escape(speech.source_show_id)}</code>.</p><ol>${speech.batches.map(batch => `<li>Stream <code>${escape(batch.stream_identity)}</code> · Plan <code>${escape(batch.plan_id)}</code> · Play <code>${escape(batch.play_id)}</code> · ${batch.speaker_blocks_committed} speaker blocks committed · ${batch.wav.audible ? 'audible PCM' : 'digital silence'} · WAV SHA-256 <code>${escape(batch.wav.sha256)}</code>.</li>`).join('')}</ol><p><a href="${speech.path}">Inspect the selected speech receipt</a>.</p></details>`));
  }
  if (report.direct_speech) {
    sections.push(chapter('direct-speech', sections.length + 1, 'Inspect a separate speech diagnostic',
      `A separate proof Host also produced ${report.direct_speech.batch_count} mechanical speech batches from source Show ${escape(report.direct_speech.source_show_id)}. That is a different Play from the speaker output above, so its WAV files are retained only as diagnostic artifacts and are not offered here as recordings of what the listener heard.`,
      '<p>The playable recordings in the preceding chapter come from the owner’s selected speaker Plays. <a href="speech-direct/manifest.json">Inspect the separate proof Host manifest</a>.</p>'));
  }
  if (report.llm_speech) {
    assert.ok(report.model_route_restoration, 'model route restoration has no producer receipt');
    const original = JSON.parse(await readFile(path.join(output, 'speech-llm',
      'original-model-output.json'))).output;
    const routeLoss = await readFile(path.join(output, 'model-route-loss.txt'), 'utf8');
    sections.push(chapter('llm-speech', sections.length + 1, 'Ask for a grounded explanation',
      'Ask the local model to explain the current Face while the three Hosts remain live. The finite Presenter validates its original wording against that Face. Its current artifact-producing proof Host is a separate Play, so this chapter shows the wording and validation but does not present those WAV files as listener audio. Then withdraw this capture’s loopback model route, observe refusal, and reopen a fresh route to the same service.',
      `<details><summary>Compare the original model output and validated text</summary><h3>Original model output</h3><pre>${escape(original)}</pre><h3>Validated text</h3><p>${escape(report.llm_speech.validated_text)}</p><p><a href="speech-llm/validation.json">Inspect validation receipt</a> · <a href="speech-llm/model-validation.json">Inspect model validation</a></p></details>`
      + `<details><summary>Inspect the configured route withdrawal</summary><p>The producer closed its own forwarding endpoint. This is a configured route refusal, not an Ollama daemon shutdown or wardrobe replacement.</p><pre>${escape(routeLoss)}</pre></details>`
      + `<details><summary>Inspect the restored model and Show</summary><p>${escape(report.model_route_restoration.validated_text)}</p><p><a href="speech-llm-restored/validation.json">Validation</a> · <a href="speech-llm-restored/speech-receipt.json">Speech receipt</a> · <a href="speech-llm-restored/manifest.json">Separate Play manifest</a></p></details>`));
  }
  if (report.screen_free_clock) {
    const clock = report.screen_free_clock;
    assert.equal(clock.body_id, report.body_id);
    assert.equal(clock.run_id, report.run_id);
    assert.equal(clock.source_commit, report.native_source_commit);
    const sessions = [];
    for (const [name, action] of [['Start', clock.start], ['Stop', clock.lull]]) {
      const input = await readFile(path.join(output, action.input.path));
      assert.equal(input.length, action.input.bytes);
      assert.equal(digest(input), action.input.sha256);
      const transcript = await readFile(path.join(output, action.transcript.path));
      assert.equal(transcript.length, action.transcript.bytes);
      assert.equal(digest(transcript), action.transcript.sha256);
      sessions.push(`<details><summary>Read the actual nonvisual ${name.toLowerCase()} session</summary><pre>${escape(transcript.toString('utf8'))}</pre></details><p><a href="${escape(action.input.path)}">Inspect the submitted ${name.toLowerCase()} commands</a></p>`);
    }
    sections.push(chapter('screen-free-clock', sections.length + 1,
      'Start and stop the clock without a screen',
      'Reenter the installed owner’s retained Body through its nonvisual interface. Read the whole current Face, focus the available Start clock action, and activate it. Read the new Face, then focus and activate Stop clock. The owner acknowledges both actions and retires the Play while retaining the same Body and Host Boot.' +
      (clock.speaker_playback_selected
        ? ' The selected speaker drained both readings; human hearing was not observed.'
        : ' This run used text readout and does not claim audio playback.'),
      `${sessions.join('')}<p><a href="report.json">Inspect the exact Face, Show, action, and result identities</a></p>`));
  }
  const ids = [...(report.birth ? ['birth'] : []), 'browser', 'native', 'native-action', 'browser-action', 'terminal-action'];
  if (report.browser_wardrobe) ids.push('wardrobe');
  if (report.owner_selected_speech) ids.push('owner-selected-speech');
  if (report.direct_speech) ids.push('direct-speech');
  if (report.llm_speech) ids.push('llm-speech');
  if (report.screen_free_clock) ids.push('screen-free-clock');
  const titles = [...(report.birth ? ['Birth'] : []), 'Browser', 'ConduitOS', 'Native action', 'Browser action', 'Terminal action'];
  if (report.browser_wardrobe) titles.push('Wardrobe');
  if (report.owner_selected_speech) titles.push('Owner-selected speech');
  if (report.direct_speech) titles.push('Direct speech');
  if (report.llm_speech) titles.push('Model explanation');
  if (report.screen_free_clock) titles.push('Screen-free clock');
  const stepLinks = ids.map((id, index) => `<li><a href="#${id}">${index + 1}. ${titles[index]}</a></li>`).join('');
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>One clock, three live Hosts · Conduit development proof</title><link rel="stylesheet" href="conduit.css"><link rel="stylesheet" href="chrome.css"><style>
body{margin:0;background:var(--conduit-background);color:var(--conduit-text-primary)}.proof{max-width:74rem;margin:auto;padding:2rem 1.25rem 5rem}.proof h1,.proof h2{font-family:var(--conduit-font-editorial);line-height:1.15}.proof h1{font-size:clamp(2.5rem,6vw,5rem);max-width:14ch}.proof h2{font-size:clamp(1.8rem,3vw,2.8rem)}.proof p{max-width:68ch}.proof .lede{font-size:1.25rem;color:var(--conduit-text-secondary)}.proof .boundary{border-left:.25rem solid var(--conduit-emphasis);padding:1rem;background:var(--conduit-reading-paper)}.proof nav ol{display:flex;flex-wrap:wrap;gap:.75rem 1.5rem;padding:0;list-style:none}.proof a{color:var(--conduit-structure-primary)}.proof a:focus-visible,.proof summary:focus-visible{outline:3px solid var(--conduit-focus);outline-offset:3px}.proof article{padding:3rem 0;border-top:1px solid var(--conduit-structure-secondary)}.proof .step{color:var(--conduit-emphasis);font-weight:700;letter-spacing:.06em;text-transform:uppercase}.proof figure{margin:1.5rem 0 2.5rem}.proof img{display:block;width:100%;height:auto;border:1px solid var(--conduit-structure-secondary);border-radius:var(--conduit-radius-panel)}.proof figcaption{padding:.6rem 0;color:var(--conduit-text-secondary)}.proof details,.proof .audio-feature{padding:1rem;background:var(--conduit-reading-paper);border:1px solid var(--conduit-structure-secondary);border-radius:var(--conduit-radius-panel)}.proof pre{max-height:30rem;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere}.proof .audio-list{padding-left:1.5rem}.proof .audio-list li{padding:1rem 0;border-top:1px solid var(--conduit-structure-secondary)}.proof audio{width:min(100%,40rem)}
</style></head><body data-application-theme="conduit.presentation/phosphor@1"><a class="conduit-skip-link" href="#proof">Skip to the proof</a>${navigation}<main id="proof" class="proof"><header><p class="step">Development proof · captured from one run</p><h1>One clock, three live Hosts</h1><p class="lede">${report.birth ? 'Birth a shared clock through nonvisual controls, then change it' : 'Change a shared clock'} from ConduitOS, a browser, and a terminal. See each result where you meet the same Body; hear its current Face through direct${report.llm_speech ? ' and validated model-assisted' : ''} speech.</p><p class="boundary">This is a live local proof, not the complete eight-chapter public journey. QMP establishes emulator execution. Produced audio is distinct from speaker playback and human listening.</p></header><nav aria-label="Proof steps"><ol>${stepLinks}</ol></nav>${sections.join('')}<details><summary>Inspect source and evidence</summary><p>Body <code>${escape(report.body_id)}</code> · source <code>${escape(report.native_source_commit)}</code> · run <code>${escape(report.run_id)}</code>.</p><p><a href="report.json">Digest-bound proof report</a> · <a href="native/owner-action-proof.json">Native QMP receipt</a>${report.birth ? ' · <a href="../birth-input.txt">Screen-free input</a> · <a href="../birth-transcript.txt">Birth transcript</a>' : ''}${report.owner_selected_speech ? ' · <a href="owner-selected-speech.json">Owner-selected playback receipt</a>' : ''}${report.direct_speech ? ' · <a href="speech-direct/manifest.json">Direct speech manifest</a>' : ''}${report.llm_speech ? ' · <a href="speech-llm/manifest.json">Model-assisted speech manifest</a>' : ''} · <a href="https://github.com/dancxjo/conduit/wiki">Project wiki</a>.</p><p>The owner retains the workload truth. This capture proves only browser-route doff, wear, explicit preference, and fresh Show under one owner Plan. It does not prove presentation-host loss and recovery, owner-sealed spoken Mask selection, or release publication.</p></details></main></body></html>`;
  const file = path.join(output, 'walkthrough.html');
  await writeFile(file, html);
  return { path: 'walkthrough.html', bytes: Buffer.byteLength(html) };
}
