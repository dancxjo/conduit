// Complete publication identities are assembled only from one finished producer run.
// Every event keeps its action-time observation; every media receipt binds
// exact producer bytes, never a fixture or a later reconstruction.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const load = async file => JSON.parse(await readFile(file, 'utf8'));

export async function writeThreeHostPublicationReceipts({ output, report, birth, start,
  finish, checkpoints, installation }) {
  const exact = (value, label) => {
    assert.ok(value !== undefined && value !== null && value !== '', `${label} is missing`);
    return value;
  };
  const observations = new Map(report.observations.map(asyncReceipt =>
    [path.basename(asyncReceipt.path, '.json'), asyncReceipt]));
  const observation = async name => {
    const artifact = exact(observations.get(name), `${name} capture observation`);
    const bytes = await readFile(path.join(output, artifact.path));
    assert.equal(digest(bytes), artifact.sha256);
    return JSON.parse(bytes);
  };
  const screenshot = file => exact(report.screenshots.find(item => item.path === file), file);
  const checkpoint = phase => exact(checkpoints.find(item => item.phase === phase), phase);
  const publicationDirectory = path.join(output, 'publication-receipts');
  await mkdir(publicationDirectory, { mode: 0o700 });
  let receiptNumber = 0;
  const emit = async (type, record) => {
    const file = `publication-receipts/${String(++receiptNumber).padStart(2, '0')}-${type}.json`;
    const bytes = Buffer.from(`${JSON.stringify({
      schema: `conduit.proof/three-host-publication-${type}@1`,
      source_commit: report.native_source_commit, run_id: report.run_id,
      body_id: report.body_id, ...record,
    }, null, 2)}\n`);
    await writeFile(path.join(output, file), bytes, { flag: 'wx', mode: 0o600 });
    return { path: file, sha256: digest(bytes) };
  };
  const event = async (kind, id, faceRevision, observedAtUnixMs, basis) => {
    exact(kind, 'event kind'); exact(id, 'event identity');
    exact(faceRevision, 'event Face revision'); exact(basis, 'event basis');
    assert.ok(Number.isSafeInteger(observedAtUnixMs) && observedAtUnixMs > 0,
      `${kind} has no action-time observation`);
    const sourceReceipt = await emit('event', {
      event_kind: kind, event_id: id,
      resulting_face_revision: String(faceRevision), outcome: 'completed',
      observed_at_unix_ms: observedAtUnixMs,
      observed_basis_sha256: digest(Buffer.from(JSON.stringify(basis))),
    });
    return { kind, id, face_revision: String(faceRevision),
      observed_at_unix_ms: observedAtUnixMs, source_receipt: sourceReceipt };
  };
  const media = async (captureSource, artifact, eventRecord, alt, basis, speech) => {
    exact(artifact?.path, 'media path'); exact(artifact?.sha256, 'media digest');
    const bytes = await readFile(path.join(output, artifact.path));
    assert.equal(digest(bytes), artifact.sha256, `${artifact.path} changed`);
    const sourceReceipt = await emit('media', {
      event_kind: eventRecord.kind, event_id: eventRecord.id,
      face_revision: eventRecord.face_revision,
      media_path: artifact.path, media_sha256: artifact.sha256,
      capture_source: captureSource,
      observed_basis_sha256: digest(Buffer.from(JSON.stringify(basis))),
    });
    return { path: artifact.path, sha256: artifact.sha256,
      capture_source: captureSource, event_id: eventRecord.id,
      face_revision: eventRecord.face_revision, alt,
      source_receipt: sourceReceipt, ...(speech ? { speech } : {}) };
  };
  const chapter = (id, title, intention, action, result, why, next,
    limitations, events, mediaItems) => ({ id, title, intention, action, result, why,
    next, limitations, events, media: mediaItems });
  const browserJoin = await observation('browser-joined');
  const browserNative = await observation('after-native-action');
  const browserAction = await observation('after-browser-action');
  const browserTerminal = await observation('after-terminal-action');
  const browserWardrobe = await observation('wardrobe-restored');
  const browserRecovery = await observation('browser-recovered');
  const nativeReceipt = await load(path.join(output, 'native/owner-action-proof.json'));
  const nativeStandby = await load(path.join(output, 'native/native-standby.json'));
  assert.equal(nativeReceipt.source_commit, report.native_source_commit);
  assert.equal(nativeReceipt.guest_part.part_id, report.guest_part_id);
  assert.equal(nativeReceipt.action.action_id, report.native_action.action_id);
  assert.equal(nativeStandby.guest_part.part_id, report.guest_part_id);
  assert.equal(nativeStandby.face.face_id, nativeReceipt.face_before.face_id);
  assert.equal(nativeStandby.face.face_revision, nativeReceipt.face_before.face_revision);
  const recovery = await load(path.join(output, report.presentation_host_recovery.path));
  assert.equal(recovery.body_id, report.body_id);
  const loss = exact(report.owner_model_route_loss, 'selected model route loss');
  const direct = exact(report.owner_direct_speech, 'direct owner speech');
  const llm = exact(report.owner_llm_speech, 'selected owner model speech');
  const originalModelOutput = (await load(path.join(output, llm.terminal.path)))
    .generation_evidence.original_model_output;
  const restoredModelOutput = (await load(path.join(output, loss.restored.terminal.path)))
    .generation_evidence.original_model_output;
  assert.equal(digest(Buffer.from(originalModelOutput)), llm.original_model_output_sha256);
  assert.equal(digest(Buffer.from(restoredModelOutput)),
    loss.restored.original_model_output_sha256);
  const observed = report.event_observed_at_unix_ms;
  assert.ok(observed, 'graphical producer omitted action-time observations');
  const born = await event('typed-interaction', birth.birth_interaction.action_id,
    birth.final_reading.face_revision, birth.observed_at_unix_ms, birth.birth_interaction);
  const joinedBrowser = await event('membership', report.browser_part_id,
    browserJoin.resulting_face.face_revision, observed.browser_join, browserJoin);
  const joinedGuest = await event('membership', report.guest_part_id,
    nativeStandby.face.face_revision,
    observed.guest_join, nativeReceipt.guest_part);
  const started = await event('typed-interaction', start.action_id,
    start.result_face_revision, start.observed_at_unix_ms, start);
  const changedNative = await event('typed-interaction', nativeReceipt.action.interaction_id,
    nativeReceipt.face_after.face_revision, observed.native_action, nativeReceipt.action);
  // The browser UI exposes the typed action and exact source Show, but does
  // not expose an invocation ID. Bind this accepted return to its real Show
  // instead of pretending the reusable action identity is a unique event.
  const changedBrowser = await event('typed-interaction-source-show',
    browserAction.cause.source_show_id,
    browserAction.resulting_face.face_revision, observed.browser_action, browserAction.cause);
  const changedTerminal = await event('typed-interaction', report.terminal_action.interaction_id,
    browserTerminal.resulting_face.face_revision, observed.terminal_action, browserTerminal.cause);
  const selectedWardrobe = await event('acknowledged-show', report.browser_wardrobe.selected_show_id,
    browserWardrobe.resulting_face.face_revision, observed.wardrobe_show, report.browser_wardrobe);
  // Each completed stream batch is a distinct listener Play. Preserve the
  // complete ordered reading, subject to the twelve-media Hear envelope
  // (one slot is reserved for the model-assisted explanation).
  assert.ok(direct.batches.length > 1 && direct.batches.length <= 11,
    'the complete direct reading exceeds the Hear chapter media bound');
  const directTerminal = await load(path.join(output, direct.terminal.path));
  assert.equal(directTerminal.speaker_playback.batches.length, direct.batches.length);
  const directAudioBytes = direct.batches.reduce((total, batch, index) => {
    const original = directTerminal.speaker_playback.batches[index];
    for (const key of ['plan_id', 'play_id', 'wav_sha256', 'pcm_sha256']) {
      assert.equal(batch[key], original[key], `direct batch ${index + 1} ${key} changed`);
    }
    assert.deepEqual(batch.spoken_segments, original.spoken_segments,
      `direct batch ${index + 1} words changed`);
    return total + batch.wav.bytes;
  }, 0);
  assert.ok(directAudioBytes + llm.wav.bytes <= 64 * 1024 * 1024,
    'complete Hear audio exceeds its bounded share of the public carrier');
  const directEvents = await Promise.all(direct.batches.map(batch =>
    event('selected-speaker-play', batch.play_id,
      direct.face_revision_decimal, batch.observed_at_unix_ms, batch)));
  const heardModel = await event('selected-speaker-play', llm.listener_play_id,
    llm.face_revision_decimal, observed.initial_model_play, llm);
  const modelLost = await event('provider-withdrawal', `route/${loss.route_plan_id}`,
    checkpoint('model-provider-unavailable').final_reading.face_revision,
    observed.model_unavailable,
    { checkpoint: checkpoint('model-provider-unavailable'),
      refusal: loss.refusal, operation_started_on_loss: loss.operation_started_on_loss });
  const modelReturned = await event('selected-speaker-play', loss.restored.listener_play_id,
    loss.restored.face_revision_decimal, observed.model_restored, loss.restored);
  const browserLost = await event('presentation-leave', recovery.lost_browser_boot_id,
    checkpoint('browser-presentation-unavailable').final_reading.face_revision,
    observed.browser_unavailable,
    recovery.loss);
  const browserReturned = await event('membership-return', recovery.recovered_browser_boot_id,
    browserRecovery.resulting_face.face_revision, observed.browser_return, recovery);
  const lulled = await event('typed-interaction', finish.action_id,
    finish.result_face_revision, finish.observed_at_unix_ms, finish);
  const qmp = (file, eventRecord, alt, face) => {
    const capture = exact(nativeReceipt.screenshots.find(item =>
      item.png === path.basename(file)), `${file} QMP capture receipt`);
    assert.equal(capture.png_sha256, screenshot(file).sha256,
      `${file} differs from the QMP producer receipt`);
    return media('qmp', screenshot(file), eventRecord,
      alt, { capture, face, source_receipt_sha256: report.native_receipt_sha256 });
  };
  const chromium = (file, eventRecord, alt, capture) => media('pinned-chromium',
    screenshot(file), eventRecord, alt, capture);
  const transcriptMedia = (artifact, eventRecord, alt) => media('screen-free-transcript',
    artifact, eventRecord, alt, artifact);
  const directSpeech = batch => ({ mode: 'direct', show_id: direct.show_id,
    plan_id: batch.plan_id, play_id: batch.play_id,
    voice_id: installation.selected_speech.voice,
    text: batch.spoken_segments.join(' '), provenance: direct.terminal });
  const modelSpeech = (record, originalOutput) => ({ mode: 'llm-assisted', show_id: record.show_id,
    plan_id: record.listener_plan_id, play_id: record.listener_play_id,
    voice_id: installation.selected_speech.voice,
    text: record.accepted_wording, provenance: record.terminal,
    original_model_output: originalOutput,
    provider_id: record.provider_identity,
    model_id: record.model_identity });
  const directMedia = await Promise.all(direct.batches.map((batch, index) =>
    media('selected-speaker-same-play', batch.wav, directEvents[index],
      `Direct spoken Face, batch ${index + 1} of ${direct.batches.length}, from its selected speaker Play`,
      batch, directSpeech(batch))));
  const publicationChapters = [
    chapter('birth', 'Make a Body', 'Create one Body without a screen.',
      'Review the clock Plot and activate Birth.', 'The installed owner retains the new Body.',
      'The other Hosts must join this same Body.', 'Open the browser and QEMU guest.',
      ['No attended human listening is claimed.'], [born],
      [await transcriptMedia(birth.transcript, born, 'Actual nonvisual Birth session')]),
    chapter('join', 'Give it more places to meet you', 'Meet the same Body on three Hosts.',
      'Join through the browser and QEMU guest.', 'Three current Parts have distinct Host and Boot identities.',
      'Membership does not silently birth another Body.', 'Set the interval.',
      ['QMP is emulator evidence, not physical hardware evidence.'],
      [joinedBrowser, joinedGuest], [
        await chromium('browser-before.png', joinedBrowser, 'Joined browser Face', browserJoin),
        await qmp('native/owner-standby.png', joinedGuest, 'Joined QEMU guest before Mask activation',
          nativeStandby.face),
      ]),
    chapter('start', 'Set the clock', 'Choose a useful interval before running the clock.',
      'Change the interval on ConduitOS, then change it in the browser.',
      'Both typed interactions change the same owner Face.',
      'The Body remains lulled while its interval changes.', 'Inspect another Mask.',
      ['This does not prove distributed workload migration.'],
      [changedNative, changedBrowser], [
        await qmp('native/owner-after.png', changedNative, 'QEMU clock after the 500 millisecond action', nativeReceipt.face_after),
        await chromium('browser-after-browser.png', changedBrowser, 'Browser clock after the 1000 millisecond action', browserAction),
      ]),
    chapter('see', 'Change how you see it', 'Meet the current work through different Masks.',
      'Use the terminal action and change browser wardrobe preference.',
      'The current Face and selected Show remain correlated.',
      'Presentation changes do not make a second Body.', 'Hear the Face.',
      ['A wardrobe choice is not a new clock action.'],
      [changedTerminal, selectedWardrobe], [
        await media('terminal', report.terminal_show, changedTerminal,
          'Installed owner terminal Show and typed clock action', report.terminal_show),
        await chromium('browser-wardrobe.png', selectedWardrobe, 'Browser wardrobe after preference change', browserWardrobe),
      ]),
    chapter('hear', 'Hear it', 'Listen to the current Face and its explanation.',
      'Select direct and model-assisted speech.',
      'The complete direct reading and selected model explanation have retained listener WAVs.',
      'The recording is from the same Play as the speaker output.', 'Withdraw a route.',
      ['Automated playback does not establish attended human hearing.'],
      [...directEvents, heardModel], [
        ...directMedia,
        await media('selected-speaker-same-play', llm.wav, heardModel,
          'Model-assisted explanation from the selected speaker Play', llm,
          modelSpeech(llm, originalModelOutput)),
      ]),
    chapter('loss', 'Change the circumstances', 'Understand a provider outage and its recovery.',
      'Withdraw the selected model route, inspect the refusal, then restore it.',
      'Model Start refuses before a new Play; a fresh selected Play completes after restoration.',
      'The unavailable provider does not erase the Body or silently invoke a fallback.',
      'Now leave and rejoin the browser.',
      ['No automatic fallback or workload failover is claimed.'],
      [modelLost, modelReturned], [
        await transcriptMedia(checkpoint('model-provider-unavailable').transcript,
          modelLost, 'Screen-free inspection during model route loss'),
        await media('selected-speaker-same-play', loss.restored.wav, modelReturned,
          'Restored model explanation from the selected speaker Play',
          loss.restored, modelSpeech(loss.restored, restoredModelOutput)),
      ]),
    chapter('return', 'Come back', 'Return to the retained Body from a fresh browser Boot.',
      'Leave the browser presentation, inspect its loss, then rejoin.',
      'Old browser actions become unavailable; the fresh Boot receives a new acknowledged Show.',
      'The owner preserves Body continuity while browser route identities change.', 'Start the clock.',
      ['No QEMU reboot or workload failover is claimed.'], [browserLost, browserReturned], [
        await transcriptMedia(checkpoint('browser-presentation-unavailable').transcript,
          browserLost, 'Screen-free inspection during browser presentation loss'),
        await chromium('browser-after-recovery.png', browserReturned,
          'Fresh browser Boot showing the same Body', browserRecovery),
      ]),
    chapter('lull', 'Run it, then leave it well', 'Run the configured clock and leave it in a known retained state.',
      'Start the clock nonvisually, then Lull it through the same controls.',
      'The clock Play runs and retires while the Body remains retained.',
      'Retained identity is distinct from continuing execution.', 'Inspect the report.',
      ['A retained Body is not proof of an active Play.'], [started, lulled], [
        await transcriptMedia(start.transcript, started, 'Nonvisual Start session'),
        await transcriptMedia(finish.transcript, lulled, 'Nonvisual Lull session'),
      ]),
  ];
  let previousObservation = 0;
  for (const chapterReceipt of publicationChapters) {
    for (const observedEvent of chapterReceipt.events) {
      assert.ok(observedEvent.observed_at_unix_ms >= previousObservation,
        `${chapterReceipt.id} reverses the producer-observed journey chronology`);
      previousObservation = observedEvent.observed_at_unix_ms;
    }
  }
  return publicationChapters;
}
