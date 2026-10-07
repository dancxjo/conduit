// A private documentary view of one producer report. Missing chapters stay visible.
// The complete public Journey has a separate strict publication gate.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { copyFile, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const escape = value => String(value).replace(/[&<>"']/g, character => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
})[character]);
const link = (href, label) => `<a href="${escape(href)}">${escape(label)}</a>`;
const details = (title, contents) => `<details><summary>${escape(title)}</summary>${contents}</details>`;
const textCapture = (value, href, title) => {
  const characters = [...value];
  const shortened = characters.length > 8_000;
  const visible = shortened ? `${characters.slice(0, 4_000).join('')}\n\n[Middle omitted; open the complete capture.]\n\n${characters.slice(-4_000).join('')}` : value;
  return details(title, `<pre>${escape(visible)}</pre><p>${link(href, 'Open complete capture')}</p>`);
};
const screenshot = (file, alt, caption) => `<figure><a href="${escape(file)}"><img loading="lazy" src="${escape(file)}" alt="${escape(alt)}"></a><figcaption>${escape(caption)}</figcaption></figure>`;
const audio = (file, words, caption) => `<figure class="audio-card"><figcaption><strong>${escape(caption)}</strong><p>${escape(words)}</p></figcaption><audio controls preload="none" src="${escape(file)}">${link(file, 'Download the recording')}</audio></figure>`;
const proof = (contents) => details('Inspect evidence and limits', contents);
const missing = reason => `<p class="missing"><strong>Not yet captured in this run.</strong> ${escape(reason)}</p>`;
const chapter = ({ id, number, title, intention, action, result, why, next, media = '', evidence = '', gap = '' }) =>
  `<article id="${id}" aria-labelledby="${id}-title"><p class="step">Chapter ${number} of 8 · ${gap ? 'incomplete' : 'captured'}</p><h2 id="${id}-title">${escape(title)}</h2><dl class="story"><dt>You want to</dt><dd>${escape(intention)}</dd><dt>Do this</dt><dd>${escape(action)}</dd><dt>What changes</dt><dd>${escape(result)}</dd><dt>Why it matters</dt><dd>${escape(why)}</dd><dt>Try next</dt><dd>${escape(next)}</dd></dl>${gap ? missing(gap) : ''}${media}${evidence ? proof(evidence) : ''}<p class="chapter-next">${number < 8 ? link(`#${CHAPTERS[number].id}`, `Continue to ${CHAPTERS[number].title} →`) : link('https://dancxjo.github.io/conduit/journeys/', 'Explore other journeys →')}</p></article>`;
const CHAPTERS = [
  { id: 'birth', title: 'Make a Body' },
  { id: 'join', title: 'Give it more places to meet you' },
  { id: 'start', title: 'Start something useful' },
  { id: 'see', title: 'Change how you see it' },
  { id: 'hear', title: 'Hear it' },
  { id: 'loss', title: 'Change the circumstances' },
  { id: 'return', title: 'Come back' },
  { id: 'lull', title: 'Leave it well' },
];
const CSS = `
body{margin:0;background:var(--conduit-background);color:var(--conduit-text-primary)}
.proof{max-width:76rem;margin:auto;padding:2rem 1.25rem 5rem}.proof h1,.proof h2{font-family:var(--conduit-font-editorial);line-height:1.15}.proof h1{font-size:clamp(2.5rem,6vw,5rem);max-width:14ch}.proof h2{font-size:clamp(1.8rem,3vw,2.8rem)}.proof p{max-width:68ch}.proof .lede{font-size:1.25rem;color:var(--conduit-text-secondary)}.proof .boundary,.proof .missing{border-left:.25rem solid var(--conduit-emphasis);padding:1rem;background:var(--conduit-reading-paper)}.proof .missing{border-color:var(--conduit-structure-secondary)}.proof nav ol{display:grid;grid-template-columns:repeat(auto-fit,minmax(13rem,1fr));gap:.5rem;padding:0;list-style:none}.proof nav a{display:block;padding:.75rem;border:1px solid var(--conduit-structure-secondary);border-radius:var(--conduit-radius-panel)}.proof a{color:var(--conduit-structure-primary)}.proof a:focus-visible,.proof summary:focus-visible{outline:3px solid var(--conduit-focus);outline-offset:3px}.proof article{padding:3rem 0;border-top:1px solid var(--conduit-structure-secondary);scroll-margin-top:2rem}.proof .step{color:var(--conduit-emphasis);font-weight:700;letter-spacing:.06em;text-transform:uppercase}.proof .story{display:grid;grid-template-columns:minmax(8rem,11rem) minmax(0,1fr);gap:.5rem 1rem;max-width:70rem}.proof .story dt{font-weight:700}.proof .story dd{margin:0 0 .5rem}.proof .chapter-next{margin-top:2rem}.proof figure{margin:1.5rem 0 2.5rem}.proof img{display:block;width:100%;max-height:42rem;object-fit:contain;border:1px solid var(--conduit-structure-secondary);border-radius:var(--conduit-radius-panel)}.proof figcaption{padding:.6rem 0;color:var(--conduit-text-secondary)}.proof details,.proof .audio-card{padding:1rem;background:var(--conduit-reading-paper);border:1px solid var(--conduit-structure-secondary);border-radius:var(--conduit-radius-panel);margin:1rem 0}.proof pre{max-height:30rem;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere}.proof audio{width:min(100%,40rem)}.proof code{overflow-wrap:anywhere}@media(max-width:650px){.proof .story{grid-template-columns:1fr}.proof .story dd{margin-bottom:.8rem}}`;

async function checked(output, artifact, expectedPath) {
  assert.ok(artifact?.path && /^[0-9a-f]{64}$/.test(artifact.sha256 ?? ''), 'capture needs a path and digest');
  const relative = expectedPath ?? artifact.path;
  assert.equal(artifact.path, relative);
  assert.ok(!path.isAbsolute(relative) && !relative.split('/').includes('..'), 'capture escapes walkthrough');
  const bytes = await readFile(path.join(output, relative));
  assert.equal(hash(bytes), artifact.sha256, `capture changed: ${relative}`);
  if (artifact.bytes !== undefined) assert.equal(bytes.length, artifact.bytes);
  return bytes;
}

export async function writeThreeHostWalkthrough(output, handbook, report) {
  assert.equal(report.schema, 'conduit.body/three-host-owner-journey@1');
  assert.match(report.proof_class, /^live-local-installed-owner-qmp-pinned-chromium/);
  assert.ok(report.run_id && report.body_id && report.native_source_commit);
  const packageHtml = await readFile(path.join(handbook, 'index.html'), 'utf8');
  const masthead = packageHtml.match(/<header class="site-header">[\s\S]*?<\/header>/)?.[0];
  assert.ok(masthead, 'exact-source Handbook has no shared site navigation');
  const navigation = masthead.replace('data-section="handbook" aria-current="page"', 'data-section="handbook"')
    .replace('data-section="journeys"', 'data-section="journeys" aria-current="page"')
    .replaceAll('href="/conduit/', 'href="https://dancxjo.github.io/conduit/');
  await Promise.all(['conduit.css', 'chrome.css'].map(name => copyFile(path.join(handbook, name), path.join(output, name))));
  const shots = new Map((report.screenshots ?? []).map(item => [item.path, item]));
  const shot = async (name, alt, caption) => {
    await checked(output, shots.get(name), name);
    return screenshot(name, alt, caption);
  };
  const clips = report.screen_free_audio;
  if (clips) {
    assert.equal(clips.proof_class, 'selected-screen-free-same-play-listener-audio-subset');
    assert.equal(clips.source_commit, report.native_source_commit);
    assert.equal(clips.human_hearing_observed, false);
    assert.equal(clips.selected.length, clips.public_selected_clip_count);
    for (const clip of clips.selected) {
      await checked(output, { path: clip.path, sha256: clip.wav_sha256, bytes: clip.bytes });
      assert.ok(clip.play_id && clip.plan_id && clip.source_show_id && clip.speaker_frames_committed > 0);
      if (clip.spoken_words !== undefined || clip.spoken_words_html !== undefined ||
          clip.spoken_segment_sha256 !== undefined) {
        assert.equal(typeof clip.spoken_words, 'string');
        assert.ok(clip.spoken_words.trim(), 'selected audio words are empty');
        assert.equal(clip.spoken_words_html, escape(clip.spoken_words));
        assert.deepEqual(clip.spoken_segment_sha256, [hash(Buffer.from(clip.spoken_words))]);
      }
    }
  }
  const hasWords = clip => typeof clip.spoken_words === 'string' && clip.spoken_words.trim() &&
    clip.spoken_words_html === escape(clip.spoken_words) &&
    clip.spoken_segment_sha256?.length === 1 &&
    clip.spoken_segment_sha256[0] === hash(Buffer.from(clip.spoken_words));
  const wordsMissing = (...moments) => (clips?.selected ?? []).some(clip =>
    moments.includes(clip.moment) && !hasWords(clip));
  const selectedAudio = moment => (clips?.selected ?? []).filter(clip => clip.moment === moment)
    .filter(hasWords).map(clip => audio(clip.path, clip.spoken_words,
      `Selected same-Play speaker output (${clip.excerpt}; Face revision ${clip.face_revision})`)).join('');
  const chapters = [];
  const birth = report.birth;
  const birthGood = birth?.proof_class?.startsWith('installed-screen-free-birth-') && birth.zero_body_observed && birth.confirmation_observed && birth.body_id === report.body_id && birth.source_commit === report.native_source_commit;
  let birthMedia = '';
  if (birthGood) {
    await checked(output, birth.zero_body_receipt, 'zero-body-before.json');
    await checked(output, birth.input, 'birth-input.txt');
    const transcript = (await checked(output, birth.transcript, 'birth-transcript.txt')).toString('utf8');
    birthMedia = textCapture(transcript, birth.transcript.path, 'Read the actual Birth session') + selectedAudio('birth-review') + selectedAudio('birth-result');
  }
  chapters.push(chapter({ ...CHAPTERS[0], number: 1, intention: 'Create the shared Body without needing a display.', action: 'Hear or read the zero-Body Crèche, choose the clock Plot and name, review the choices, then explicitly activate Birth.', result: birthGood ? 'The installed Linux Host retains one Body on its current Boot.' : 'No completed Birth result is available in this report.', why: 'The later screens must join this Body, rather than present independent births as one.', next: 'Open the browser and ConduitOS.', media: birthMedia, gap: !birthGood ? 'A producer-owned zero-Body and explicit Birth record is required.' : wordsMissing('birth-review', 'birth-result') ? 'Selected Birth WAVs lack their exact producer-verified spoken words, so those controls are withheld.' : '', evidence: birthGood ? `${link('zero-body-before.json', 'Zero-Body receipt')} · ${link('birth-input.txt', 'Actual commands')}. Selected audio is a subset of completed speaker Plays; it does not prove human hearing or owner-instance speech realization.` : '' }));

  const joined = report.concurrent_part_count === 3 && report.qemu_alive_through_browser_actions === true && report.browser_host_id && report.guest_host_id && report.owner_host_id && report.browser_part_id && report.guest_part_id;
  let joinMedia = '';
  if (joined) {
    await checked(output, { path: 'native/owner-action-proof.json', sha256: report.native_receipt_sha256 });
    joinMedia = await shot('browser-before.png', 'Browser showing the joined Body before the clock changes', 'The browser sees the owner Face.') + await shot('native/owner-standby.png', 'ConduitOS guest awaiting Mask selection', 'QMP capture from the running guest.');
  }
  chapters.push(chapter({ ...CHAPTERS[1], number: 2, intention: 'Meet the same Body from two more live places.', action: 'Admit the browser and QEMU guest through their supported joining flows.', result: joined ? 'Three distinct Host and Boot identities participate in this Body.' : 'A correlated three-host result is missing.', why: 'Host membership and Body identity are separate facts.', next: 'Start or change the clock.', media: joinMedia, gap: joined ? '' : 'The report must identify admitted browser and guest Parts alongside the installed owner.', evidence: joined ? `Browser Host <code>${escape(report.browser_host_id)}</code>; guest Host <code>${escape(report.guest_host_id)}</code>; installed owner <code>${escape(report.owner_host_id)}</code>. QMP proves emulator execution, not physical hardware. ${link('native/owner-action-proof.json', 'Guest action proof')}.` : '' }));

  const clock = report.screen_free_clock;
  const started = clock?.proof_class?.startsWith('installed-screen-free-clock-') && clock.body_id === report.body_id && clock.run_id === report.run_id && clock.start?.result_face_id && report.native_action?.status === 'accepted' && report.browser_action?.status === 'accepted';
  let startMedia = '';
  if (started) {
    await checked(output, clock.start.input, 'clock-start-input.txt');
    const transcript = (await checked(output, clock.start.transcript, 'clock-start-transcript.txt')).toString('utf8');
    startMedia = textCapture(transcript, clock.start.transcript.path, 'Read the nonvisual Start session') + await shot('native/owner-after.png', 'ConduitOS showing the changed clock', 'The guest accepts the typed 500 millisecond action.') + await shot('browser-after-native.png', 'Browser showing the same 500 millisecond clock', 'A second Host sees the updated Face.') + await shot('browser-after-browser.png', 'Browser after its own 1000 millisecond action', 'The browser then changes the shared clock.') + selectedAudio('clock-start');
  }
  chapters.push(chapter({ ...CHAPTERS[2], number: 3, intention: 'Get a useful clock running and change its interval.', action: 'Start from the nonvisual controls, then use ConduitOS and browser clock controls.', result: started ? 'The owner accepts typed actions and each graphical Face shows the resulting interval.' : 'The required start and graphical action sequence is not complete.', why: 'The clock is shared work on the installed owner; another Host does not silently become its owner.', next: 'Inspect it through different Masks.', media: startMedia, gap: !started ? 'Need one correlated Start and accepted graphical actions with current captures.' : wordsMissing('clock-start') ? 'Selected Start WAV lacks its exact producer-verified spoken words, so its control is withheld.' : '', evidence: started ? `${link(clock.start.input.path, 'Nonvisual Start input')} · ${link('native/owner-action-proof.json', 'Native action receipt')} · ${link('report.json', 'Exact Face revisions')}. The Linux owner retains the workload.` : '' }));

  const terminal = report.terminal_show;
  const seeing = terminal?.path && report.terminal_action?.status === 'accepted' && report.browser_wardrobe?.selected_show_id;
  let seeMedia = '';
  if (seeing) {
    await checked(output, report.browser_wardrobe, 'browser-wardrobe.json');
    const action = (await checked(output, terminal, 'terminal-face.txt')).toString('utf8');
    const setup = terminal.setup_path
      ? textCapture((await checked(output, { path: terminal.setup_path, sha256: terminal.setup_sha256 }, 'terminal-setup.txt')).toString('utf8'), terminal.setup_path, 'Read terminal wardrobe setup') : '';
    if (report.screen_free_wardrobe?.transcript) await checked(output, report.screen_free_wardrobe.transcript, 'wardrobe-transcript.txt');
    seeMedia = await shot('native/owner-before.png', 'Selected native Mask on ConduitOS', 'QMP shows the selected owner Face.') + await shot('browser-after-terminal.png', 'Browser after a terminal action', 'The browser confirms the terminal change.') + setup + textCapture(action, terminal.path, 'Read the terminal action') + selectedAudio('wardrobe-inspection') + selectedAudio('wardrobe-preference');
  }
  chapters.push(chapter({ ...CHAPTERS[3], number: 4, intention: 'Inspect the same work through graphical, terminal, and nonvisual views.', action: 'Wear and prefer admitted Masks, inspect the Face, and enter the terminal interval action.', result: seeing ? 'The terminal action reaches the installed owner and a fresh route Plan acknowledges the changed Face.' : 'A correlated terminal and wardrobe result is missing.', why: 'A preference chooses among admitted routes; terminal replacement planning is recorded separately.', next: 'Listen to a direct reading and grounded explanation.', media: seeMedia, gap: !seeing ? 'Need selected graphical and terminal Shows plus a typed terminal action.' : wordsMissing('wardrobe-inspection', 'wardrobe-preference') ? 'Selected wardrobe WAVs lack exact producer-verified spoken words, so those controls are withheld.' : '', evidence: seeing ? `${link('browser-wardrobe.json', 'Owner wardrobe transitions')} · ${link('report.json', 'Plan and Show identities')}. ${report.screen_free_wardrobe ? `${link('wardrobe-transcript.txt', 'Nonvisual wardrobe session')}.` : ''} This does not imply Host loss or automatic fallback.` : '' }));

  const direct = report.owner_direct_speech;
  const readAloud = report.owner_selected_speech;
  const model = report.owner_llm_speech;
  const directGood = direct?.proof_class === 'installed-owner-direct-mask-and-same-play-speaker' && direct.run_id === report.run_id && direct.body_id === report.body_id && direct.source_commit === report.native_source_commit && direct.batches?.length > 1;
  const modelGood = model?.proof_class === 'installed-owner-selected-model-and-same-play-speaker' && model.run_id === report.run_id && model.body_id === report.body_id && model.source_commit === report.native_source_commit;
  let hearMedia = '';
  if (directGood) {
    const terminalReceipt = JSON.parse((await checked(output, direct.terminal)).toString('utf8'));
    assert.equal(terminalReceipt.show_id, direct.show_id);
    for (const batch of direct.batches) await checked(output, batch.wav);
    hearMedia += audio(direct.batches[0].wav.path, direct.batches[0].spoken_segments.join(' '), 'Direct Mask reading from a selected speaker Play')
      + details('Inspect the complete direct reading', `<ol>${direct.batches.map(batch => `<li>${audio(batch.wav.path, batch.spoken_segments.join(' '), `Plan ${batch.plan_id} · Play ${batch.play_id}`)}</li>`).join('')}</ol><p>${link(direct.terminal.path, 'Owner Mask and speaker receipt')}</p>`);
  } else if (readAloud?.proof_class === 'installed-owner-selected-speaker-playback' && readAloud.wav_artifact_from_this_play === true && readAloud.body_id === report.body_id) {
    const batch = readAloud.batches?.find(item => item.wav?.audible && item.wav.source === 'same-selected-speaker-play');
    if (batch) {
      await checked(output, batch.wav);
      hearMedia += audio(batch.wav.path, batch.spoken_segments.join(' '), 'Owner-selected Read this view aloud Play');
    }
  }
  if (modelGood) {
    const terminalReceipt = JSON.parse((await checked(output, model.terminal)).toString('utf8'));
    assert.equal(terminalReceipt.schema, 'conduit.body/owner-spoken-terminal@1');
    assert.equal(terminalReceipt.speaker_played, true);
    assert.equal(terminalReceipt.speaker_playback.play_id, model.listener_play_id);
    assert.equal(terminalReceipt.speaker_playback.source_show_id, model.show_id);
    assert.equal(hash(Buffer.from(terminalReceipt.generation_evidence.original_model_output)), model.original_model_output_sha256);
    await checked(output, model.wav);
    hearMedia += audio(model.wav.path, model.accepted_wording, 'Model explanation from the selected speaker Play') + details('Compare original model words and validated wording', `<h3>Original model output</h3><pre>${escape(terminalReceipt.generation_evidence.original_model_output)}</pre><h3>Words admitted for this Face</h3><p>${escape(model.accepted_wording)}</p><p>${link(model.terminal.path, 'Validation and owner Show receipt')}</p>`);
  }
  chapters.push(chapter({ ...CHAPTERS[4], number: 5, intention: 'Hear the current Face and ask it to explain itself.', action: 'Select the direct spoken Mask, then the model-assisted spoken Mask.', result: directGood && modelGood ? 'Both selected speaker paths complete; model wording is retained and validated against the Face.' : directGood ? 'Direct Mask speaker output is captured; model-assisted selected-speaker output is still missing.' : 'The two spoken Masks are not both captured.', why: 'A separately synthesized WAV cannot stand in for PCM delivered in the selected speaker Play.', next: 'Change a provider or presentation Host and inspect the outcome.', media: hearMedia, gap: directGood && modelGood ? '' : 'Full chapter needs both direct and model-assisted same-Play Mask audio. Any available Read this view aloud recording is a narrower partial result.', evidence: `${directGood ? `${link(direct.terminal.path, 'Selected direct Mask receipt')}. ` : ''}${readAloud && !directGood ? `${link(readAloud.path, 'Owner-selected Read this view aloud receipt')}. ` : ''}${modelGood ? `${link(model.terminal.path, 'Original model output and validation')}. ` : ''}Attended human hearing is not established; diagnostic WAVs from a separate Play are excluded.` }));

  const loss = report.owner_model_route_loss;
  const lossGood = loss?.proof_class === 'installed-owner-selected-model-route-withdrawal-and-restoration' && loss.run_id === report.run_id && loss.body_id === report.body_id && loss.operation_started_on_loss === false && loss.owner_face_unchanged === true && loss.new_listener_wav_on_failure === false;
  if (lossGood) {
    await checked(output, loss.refusal);
    await checked(output, loss.wardrobe);
    await checked(output, loss.restored.terminal);
  }
  const host = report.presentation_host_recovery;
  let hostReceipt = null;
  if (host?.path) {
    hostReceipt = JSON.parse((await checked(output, host)).toString('utf8'));
    assert.equal(hostReceipt.schema, 'conduit.proof/browser-presentation-recovery@1');
    assert.equal(hostReceipt.source_commit, report.native_source_commit);
    assert.equal(hostReceipt.run_id, report.run_id);
    assert.equal(hostReceipt.body_id, report.body_id);
    assert.equal(hostReceipt.lost_browser_boot_id, host.lost_boot_id);
    assert.equal(hostReceipt.recovered_browser_boot_id, host.recovered_boot_id);
    assert.equal(hostReceipt.old_show_id, host.old_show_id);
    assert.equal(hostReceipt.recovered_show_id, host.recovered_show_id);
  }
  const hostGood = hostReceipt && hostReceipt.old_show_id !== hostReceipt.recovered_show_id &&
    hostReceipt.lost_browser_boot_id !== hostReceipt.recovered_browser_boot_id &&
    hostReceipt.loss?.presence === 'unavailable' && hostReceipt.loss.actionDisabled === true;
  const lossMedia = lossGood ? `<p>${link(loss.refusal.path, 'Owner refusal receipt')} · ${link(loss.wardrobe.path, 'Wardrobe before, during, and after provider withdrawal')}</p>` : '';
  chapters.push(chapter({ ...CHAPTERS[5], number: 6, intention: 'See what remains available when a presentation Host or model route disappears.', action: 'Close the browser presentation and withdraw the selected model endpoint.', result: hostGood && lossGood ? 'Old browser actions become stale; the selected model Mask refuses before a new Play.' : 'Only part of the required loss sequence is captured.', why: 'Availability, authority, preference, and fresh planning have distinct outcomes.', next: 'Restore the lost route and return through a fresh Boot.', media: lossMedia, gap: hostGood && lossGood ? '' : `Missing ${hostGood ? '' : 'browser presentation-host loss'}${!hostGood && !lossGood ? ' and ' : ''}${lossGood ? '' : 'owner-selected model route refusal'}. A configured diagnostic model endpoint is not a substitute.`, evidence: lossGood ? `${link(loss.refusal.path, 'No-operation refusal')}. The model service itself remains running; no automatic fallback or replacement Plan is claimed.` : '' }));

  let returnMedia = '';
  if (hostGood) returnMedia += await shot('browser-after-recovery.png', 'Fresh browser Boot showing the retained Body', 'The returned browser receives a new acknowledged Show.');
  if (lossGood) {
    await checked(output, loss.restored.wav);
    returnMedia += audio(loss.restored.wav.path, loss.restored.accepted_wording, 'Restored selected speaker Play');
  }
  chapters.push(chapter({ ...CHAPTERS[6], number: 7, intention: 'Return to the same Body after the interruption.', action: 'Reopen the browser on a fresh Boot and restore the private model endpoint.', result: hostGood && lossGood ? 'New browser Show and model speaker Play replace their stale predecessors.' : 'The full return sequence is not captured.', why: 'The installed owner retains the Body and clock; return requires fresh admission and Show facts.', next: 'Lull the clock and inspect its retained state.', media: returnMedia, gap: hostGood && lossGood ? '' : 'Need both fresh browser Boot/Show evidence and a restored selected model speaker Play.', evidence: `${hostGood ? `${link(host.path, 'Browser loss and fresh-Boot receipt')}. ` : ''}${lossGood ? `${link(loss.restored.terminal.path, 'Restored model Show and Play')}. ` : ''}This does not prove QMP reboot or workload failover.` }));

  const lulled = clock?.lull?.result_face_id && clock.body_id === report.body_id && clock.run_id === report.run_id;
  let lullMedia = '';
  if (lulled) {
    await checked(output, clock.lull.input, 'clock-lull-input.txt');
    lullMedia = textCapture((await checked(output, clock.lull.transcript, 'clock-lull-transcript.txt')).toString('utf8'), clock.lull.transcript.path, 'Read the nonvisual Lull session') + selectedAudio('clock-lull');
  }
  chapters.push(chapter({ ...CHAPTERS[7], number: 8, intention: 'Leave the clock in a known, retained state.', action: 'Use nonvisual controls to Lull and read the resulting Face.', result: lulled ? 'The installed owner retires the clock Play while retaining the Body.' : 'No correlated Lull result is available.', why: 'A retained Body is different from a running clock or a live presentation Show.', next: 'Inspect the receipts or follow another journey.', media: lullMedia, gap: !lulled ? 'Need the producer-owned typed Lull action, resulting Face, and user-visible capture.' : wordsMissing('clock-lull') ? 'Selected Lull WAV lacks its exact producer-verified spoken words, so its control is withheld.' : '', evidence: lulled ? `${link(clock.lull.input.path, 'Submitted Lull command')} · ${link('report.json', 'Exact Play and Face result')}. Selected audio is a subset; attended listening is not established.` : '' }));

  const completed = chapters.filter(html => html.includes('· captured</p>')).length;
  const links = CHAPTERS.map((item, index) => `<li>${link(`#${item.id}`, `${index + 1}. ${item.title}`)}</li>`).join('');
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>One Body, three Hosts · Conduit development Journey</title><link rel="stylesheet" href="conduit.css"><link rel="stylesheet" href="chrome.css"><style>${CSS}</style></head><body data-application-theme="conduit.presentation/phosphor@1"><a class="conduit-skip-link" href="#journey">Skip to the journey</a>${navigation}<main id="journey" class="proof"><header><p class="step">Development capture · ${completed} of 8 chapters complete</p><h1>One Body, three Hosts, five ways to meet it</h1><p class="lede">Follow a clock from its first Birth through graphical, terminal, and spoken encounters. Each chapter shows the action, visible result, and exact boundary of this recorded run.</p><p class="boundary">This page records one local development run; incomplete chapters are marked. QMP establishes emulator execution. Same-Play audio establishes completed device output, not attended human hearing. The public Journey and accepted release require separate gates.</p></header><nav aria-label="Journey chapters"><h2>Follow the journey</h2><ol>${links}</ol></nav>${chapters.join('')}<details><summary>Source and complete report</summary><p>Body <code>${escape(report.body_id)}</code> · source <code>${escape(report.native_source_commit)}</code> · run <code>${escape(report.run_id)}</code>.</p><p>${link('report.json', 'Producer report')} · ${link('https://github.com/dancxjo/conduit/issues/4807', 'Journey acceptance issue')}.</p><p>Diagnostic artifacts and private playback receipts remain distinct from the selected public audio subset. Missing chapters do not become completed by adding commentary or fixtures.</p></details></main></body></html>`;
  await writeFile(path.join(output, 'walkthrough.html'), html);
  return { path: 'walkthrough.html', bytes: Buffer.byteLength(html) };
}
