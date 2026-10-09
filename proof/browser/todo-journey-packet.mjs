// Internal finalizer for one successful supported live Todo producer invocation.
// Documentary validation cannot turn arbitrary input into live acceptance.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { validateTodoJourney } from '../../tools/ci/pipeline/todo-journey.mjs';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const read = file => JSON.parse(readFileSync(file));
const textType = 'text/plain; charset=utf-8';

export function finalizeTodoJourney(captureRoot) {
  const raw = read(path.join(captureRoot, 'live-run.json'));
  assert.equal(raw.capture_entrance, 'cargo xtask prove todo-journey');
  assert.equal(raw.error, null);
  assert.equal(raw.capture_tool_commit, raw.installed_product_source_commit);
  const observation = raw.observation;
  const browser = read(path.join(captureRoot, 'browser/receipt.json'));
  assert.equal(browser.source_relation, 'exact-source');
  assert.equal(browser.owner_source_commit, raw.capture_tool_commit);
  assert.equal(browser.body_id, observation.body_id);
  assert.equal(browser.adds.length, 3);
  assert.ok(observation.checkpoint_refusal, 'complete producer must retain its failure and repair');
  const speech = browser.direct_speech;
  const recovery = observation.recovery;
  assert.ok(speech && recovery);
  const root = path.join(captureRoot, 'publication');
  mkdirSync(root, { mode: 0o700 }); // Existing evidence is never overwritten.
  const identity = { source_commit: raw.capture_tool_commit,
    run_id: `todo/${digest(Buffer.from(`${raw.capture_tool_commit}/${observation.body_id}/${raw.started_at_unix_ms}`))}`,
    body_id: observation.body_id };
  const outputs = [];
  const add = (id, kind, bytes, mediaType, extension = 'json') => {
    const data = Buffer.isBuffer(bytes) ? bytes : Buffer.from(JSON.stringify(bytes, null, 2));
    assert.ok(data.length > 0 && data.length <= 16 * 1024 * 1024);
    const relative = `${id}.${extension}`;
    writeFileSync(path.join(root, relative), data, { flag: 'wx' });
    const output = { id, kind, path: relative, media_type: mediaType, required: true,
      bytes: data.length, sha256: digest(data), scenario_id: identity.run_id,
      proof_class: 'producer-correlated-live-local' };
    outputs.push(output);
    return output;
  };
  const document = (id, value) => add(id, 'machine-readable-manifest', value, 'application/json');
  const stories = {
    birth: ['Start a list', 'Keep a grocery list.', 'Birth one Body named Groceries.',
      'The terminal acknowledges its first available Show.', 'The list has one identity before any items are added.', 'Add what you need.'],
    add: ['Add three things', 'Remember three errands.', 'Add items in the browser checklist.',
      browser.adds.map(item => item.text).join(' · '), 'Each accepted action commits the same list.', 'Join the list in a terminal.'],
    join: ['Meet the same list', 'Use the list from another Mask.', 'Wear and prefer the terminal Mask.',
      'The terminal shows the three browser-added items.', 'Changing the presentation preserves the list.', 'Complete the first errand.'],
    complete: ['Finish one errand', 'Mark an item done.', `Complete ${browser.item_text} in the terminal.`,
      'The checkpoint commits the completed item.', 'A terminal action changes the Body seen by the browser.', 'Refresh the browser to inspect the result.'],
    inspect: ['See what remains', 'Check the updated list.', 'Refresh the browser after the terminal completion.',
      '2 things left · 1 completed', 'The completed item stays subordinate to remaining work.', 'Request a spoken opening.'],
    hear: ['Hear the overview', 'Get a short spoken status.', 'Select and start the direct spoken Mask.',
      speech.opening.direct_opening_wording, 'The opening reports current list truth without reading every detail.', 'Request the remaining items explicitly.'],
    read: ['Read the remaining items', 'Hear every remaining item.', 'Request the current items through the spoken Mask reader.',
      speech.remaining.batches.flatMap(batch => batch.spoken_segments).join(' '),
      'Finite batches cover the remaining items once, in order.', 'Reencounter the list after the Owner stops.'],
    recover: ['Return to your list', 'Keep the list across a fresh Owner Boot.', 'Restart the installed Owner and join from a new browser Host.',
      'The same Body restores the exact committed state.', 'A new Boot verifies the checkpoint instead of trusting a remembered list.', 'Continue using the list.'],
  };
  const chapters = [];
  const events = [];
  const mediaIds = [];
  const chapter = (id, stamp, extra, mediaSpecs) => {
    const event = { ...identity, schema: 'conduit.todo-journey/producer-event@1',
      chapter_id: id, event_id: `${identity.run_id}/${id}`, ...stamp, ...extra, media: [] };
    assert.equal(typeof event.face_revision, 'string');
    const media = mediaSpecs.map((spec, index) => {
      const outputId = `${id}-media-${index}`;
      const audio = spec.audio;
      const kind = audio ? 'audio' : spec.source === 'terminal' ? 'console-transcript' : 'screenshot';
      const output = add(outputId, kind, readFileSync(path.join(captureRoot, spec.file)),
        audio ? 'audio/wav' : kind === 'screenshot' ? 'image/png' : textType,
        audio ? 'wav' : kind === 'screenshot' ? 'png' : 'txt');
      let delivery;
      if (audio) {
        const transcript = add(`${outputId}-transcript`, 'document', Buffer.from(audio.text), textType, 'txt');
        delivery = { ...identity, face_id: event.face_id, face_revision: event.face_revision,
          show_id: event.show_id, play_id: audio.play, plan_id: audio.plan,
          host_id: audio.host, boot_id: audio.boot, provider_sha256: audio.provider,
          wav_artifact_id: audio.locator, artifact_pcm_sha256: audio.pcm,
          wav_sha256: output.sha256, channels: 2, sample_rate_hz: 48000, bits_per_sample: 16,
          output_mode: 'wav-artifact', outcome: 'completed', speaker_frames_committed: 0,
          speaker_blocks_committed: 0, physical_playback: false, human_listening: false,
          spoken_text_sha256: transcript.sha256, transcript_output_id: transcript.id,
          transcript_sha256: transcript.sha256, speech_mode: 'direct', voice_id: speech.selected_voice };
      }
      event.media.push({ media_output_id: outputId, capture_source: spec.source,
        media_sha256: output.sha256, ...(delivery ? { audio: delivery } : {}) });
      document(`${outputId}-receipt`, { ...identity, schema: 'conduit.todo-journey/capture-receipt@1',
        chapter_id: id, event_id: event.event_id, face_id: event.face_id,
        face_revision: event.face_revision, show_id: event.show_id, source_receipt_id: `${id}-source`,
        media_output_id: outputId, media_sha256: output.sha256, capture_source: spec.source, ...delivery });
      mediaIds.push(outputId);
      return { output_id: outputId, receipt_id: `${outputId}-receipt`, alt: spec.alt };
    });
    document(`${id}-source`, event);
    document(`${id}-receipt`, { ...event, schema: 'conduit.todo-journey/chapter-receipt@1', source_receipt_id: `${id}-source` });
    events.push(event.event_id);
    const [title, intention, action, result, why, next] = stories[id];
    chapters.push({ id, title, intention, action, result, why, next, media, receipt_id: `${id}-receipt`,
      limitations: id === 'hear' || id === 'read'
        ? ['Selected WAV artifacts; zero speaker delivery and no attended human listening.']
        : ['Local installed Linux Owner; distinct browser Hosts share this physical machine.'] });
  };
  const stamp = evidence => ({ face_id: evidence.face_id, face_revision: evidence.face_revision, show_id: evidence.show_id });
  const born = observation.birth.show;
  chapter('birth', { ...stamp(born.evidence), observed_at_unix_ms: born.observed_at_unix_ms }, {},
    [{ source: 'terminal', file: 'birth-show.stdout', alt: 'The newly born Groceries list in its terminal Show' }]);
  const lastAdd = browser.adds.at(-1);
  const mutation = value => ({ interaction_id: value.interaction_id, action_id: value.action_id,
    queue_sequence: value.queue_sequence, child_sign_id: value.write_terminal_sign.sign_id, outcome: 'produced' });
  chapter('add', { face_id: lastAdd.after.face_id, face_revision: lastAdd.owner.presentation_revision_decimal,
    show_id: lastAdd.after.show_id, observed_at_unix_ms: lastAdd.mutation.observed_at_unix_ms },
  { ...mutation(lastAdd.mutation), mask_kind: 'chromium' },
  [{ source: 'chromium', file: `browser/${lastAdd.screenshot}`, alt: 'Three items added to the browser checklist' }]);
  const cross = browser.cross_mask;
  chapter('join', { ...stamp(cross.joined.evidence), observed_at_unix_ms: cross.joined.observed_at_unix_ms }, {},
    [{ source: 'terminal', file: 'browser/terminal-join.stdout', alt: 'The same three items encountered in the terminal' }]);
  chapter('complete', { ...stamp(cross.completed_evidence), observed_at_unix_ms: cross.completion.observed_at_unix_ms },
    { ...mutation(cross.completion), mask_kind: 'terminal' },
    [{ source: 'terminal', file: 'browser/terminal-complete.stdout', alt: 'Terminal completion of the first item' }]);
  chapter('inspect', browser.inspect_capture, {},
    [{ source: 'chromium', file: 'browser/browser-after.png', alt: 'Two items remain and one completed item is collapsed' }]);
  const spokenStamp = { face_id: speech.selected_face_id, face_revision: speech.selected_face_revision, show_id: speech.opening.show_id };
  const artifact = speech.opening.artifact;
  chapter('hear', { ...spokenStamp, observed_at_unix_ms: speech.opening_observed_at_unix_ms }, {},
    [{ source: 'selected-wav-artifact', file: 'browser/spoken/opening.wav', alt: 'Selected direct spoken opening recording',
      audio: { play: artifact.active_play_id, plan: artifact.plan_id, host: speech.selected_host_id,
        boot: speech.selected_boot_id, provider: speech.selected_provider_sha256,
        locator: path.basename(artifact.artifact_locator), pcm: artifact.content_sha256, text: speech.opening.direct_opening_wording } }]);
  chapter('read', { ...spokenStamp, observed_at_unix_ms: speech.remaining_observed_at_unix_ms },
    { reader_command: 'read-current-items', mask_kind: 'direct-spoken', mask_play_id: speech.opening.active_play_id },
    speech.remaining.batches.map((batch, index) => ({ source: 'selected-wav-artifact',
      file: `browser/spoken/remaining-${index}.wav`, alt: `Remaining-item recording ${index + 1}`,
      audio: { play: batch.play_id, plan: batch.plan_id, host: speech.remaining.host_id,
        boot: speech.remaining.boot_id, provider: batch.provider_sha256, locator: batch.wav_artifact_id,
        pcm: batch.pcm_sha256, text: batch.spoken_segments.join(' ') } })));
  chapter('recover', { ...stamp(recovery.browser), observed_at_unix_ms: recovery.observed_at_unix_ms },
    { previous_boot_id: recovery.previous_boot_id, new_boot_id: recovery.new_boot_id,
      pre_lull_state_sha256: recovery.pre_lull_state_sha256, recovered_state_sha256: recovery.recovered_state_sha256 },
    [{ source: 'chromium', file: 'recovery/browser/browser-reencounter.png', alt: 'The same list reencountered after a fresh Owner Boot' }]);
  document('live-run', raw);
  document('browser-run', browser);
  document('checkpoint-refusal', read(path.join(captureRoot, 'checkpoint-refusal/refused-execution.json')));
  add('checkpoint-refusal-visible', 'document', readFileSync(path.join(captureRoot, 'checkpoint-refusal/refused.stderr')), textType, 'txt');
  add('stale-action-visible', 'screenshot', readFileSync(path.join(captureRoot, 'browser/browser-stale.png')), 'image/png', 'png');
  document('recovery-read', read(path.join(captureRoot, 'recovery/owner-execution.json')));
  document('repaired-read', read(path.join(captureRoot, 'checkpoint-refusal/recovery/owner-execution.json')));
  document('producer-terminal', { ...identity, schema: 'conduit.todo-journey/producer-terminal@1',
    capture_entrance: raw.capture_entrance, outcome: 'completed', event_ids: events, media_output_ids: mediaIds });
  document('journey', { ...identity, schema: 'conduit.journey/todo@1',
    producer_terminal_receipt_id: 'producer-terminal', chapters });
  writeFileSync(path.join(root, 'manifest.json'), JSON.stringify({ schema: 'conduit.evidence-manifest/v1',
    result: 'complete', git_commit: identity.source_commit, proof_id: 'journey-todo-one-body', suite_id: 'journey-gallery', outputs }, null, 2), { flag: 'wx' });
  validateTodoJourney(root, identity.source_commit);
  return root;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  assert.equal(process.argv.length, 3, 'internal finalizer requires one new live capture root');
  console.log(finalizeTodoJourney(path.resolve(process.argv[2])));
}
