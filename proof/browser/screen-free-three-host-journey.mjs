// Producer-owned journey: an installed zero-Body Host receives real nonvisual
// Birth commands before its one Body is provisioned for the three-host proof.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, existsSync, openSync } from 'node:fs';
import { mkdir, readFile, rename, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { writeThreeHostWalkthrough } from './three-host-walkthrough.mjs';
import { makeZeroBodyReceipt } from './zero-body-receipt.mjs';
import { runPacedScreenFree } from './paced-screen-free-input.mjs';
import { screenFreeWardrobeCommands, verifyScreenFreeWardrobe } from './screen-free-wardrobe-proof.mjs';
import { retainSelectedScreenFreePlays } from './screen-free-same-play.mjs';
import { checkpointPhases, verifyCheckpointReady,
  verifyCheckpointWardrobe } from './screen-free-checkpoint-proof.mjs';
import { retainScreenFreeSessions } from './three-host-retain-screen-free.mjs';
import { verifyWalkthroughAssets } from './three-host-walkthrough-assets.mjs';
import { verifyGuestRouteCertificate } from './three-host-route-certificate.mjs';

const [xtaskArg, ownerArg, stateArg, handbookArg, buildArg, profileArg,
  certArg, keyArg, forward, routeUrl, outputArg, playwrightArg, bodyName,
  speakerCardArg, speakerDeviceArg, speechExecutableArg, speechDataArg, speechEngineArg,
  speechLanguageCoverageArg, modelArg, modelEndpoint, modelMemory,
  ownerModelRouteControlArg] = process.argv.slice(2);
const [speakerCard, speakerDevice, speechExecutable, speechData, speechEngine, speechLanguageCoverage] =
  [speakerCardArg, speakerDeviceArg, speechExecutableArg, speechDataArg, speechEngineArg, speechLanguageCoverageArg]
    .map(value => value === '-' ? undefined : value);
const model = modelArg === '-' ? undefined : modelArg;
assert.ok(bodyName && !bodyName.includes('\n') && !bodyName.includes('\r') &&
  Buffer.byteLength(bodyName) <= 120, 'Body name must be one bounded input line');
assert.equal(Boolean(speakerCard), Boolean(speakerDevice));
assert.ok(!speakerCard || speechExecutable, 'selected speaker needs a speech provider');
assert.equal(Boolean(speechExecutable), Boolean(speechData));
assert.equal(Boolean(speechExecutable), Boolean(speechEngine));
assert.equal(Boolean(speechExecutable), Boolean(speechLanguageCoverage));
assert.ok(!model || speechExecutable, 'model-assisted speech needs the selected speech provider');
assert.equal(Boolean(model), Boolean(modelEndpoint));
assert.equal(Boolean(model), Boolean(modelMemory));
const [xtask, owner, state, handbook, build, profile, cert, key, output, playwright] =
  [xtaskArg, ownerArg, stateArg, handbookArg, buildArg, profileArg,
    certArg, keyArg, outputArg, playwrightArg].map(value => path.resolve(value));
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const load = async file => JSON.parse(await readFile(file, 'utf8'));
const selectedSpeechArgs = speakerCard ? ['--speak', '--speaker-card', speakerCard,
  '--speaker-device', speakerDevice, '--speech-executable', speechExecutable,
  '--speech-data', speechData, '--speech-engine', speechEngine,
  '--speech-language-coverage', speechLanguageCoverage] : [];
// Full-Face playback is serialized at the selected ALSA device. The scripted
// Birth and clock sessions each include multiple complete readings, not a
// single generated artifact; retain a finite wall-clock deadline for them.
const screenFreeSessionTimeout = speakerCard ? 30 * 60_000 : 30_000;
const attestReading = (transcript, snapshot, part, label, allowStaleCancellation = false) => {
  const face = snapshot.presentation;
  const revision = snapshot.presentation_revision_decimal;
  const receipts = transcript.split('\n').flatMap(line => {
    const start = line.indexOf('{"');
    if (start < 0) return [];
    try { return [JSON.parse(line.slice(start))]; } catch { return []; }
  });
  const turns = receipts.filter(item => item.schema === 'conduit.body/spoken-face-turn@1');
  const plays = receipts.filter(item => item.schema === 'conduit.body/spoken-face-playback@1');
  if (speakerCard) {
    assert.ok(turns.length > 0 && plays.length > 0,
      `${label} selected speaker produced no completed spoken turn and playback`);
    for (const played of plays) {
      assert.equal(played.outcome, 'Completed');
      assert.equal(played.speaker_lifecycle, 'StoppedClosed');
      assert.equal(played.host_id, part.host_id);
      assert.equal(played.boot_id, part.boot_id);
      assert.match(played.face_revision_decimal, /^(0|[1-9][0-9]*)$/);
      assert.ok(played.speaker_frames_committed > 0);
      assert.equal(played.speaker_underruns, 0);
    }
    for (const turn of turns) {
      assert.ok(turn.outcome === 'Completed' ||
        (allowStaleCancellation && turn.outcome === 'Cancelled' &&
          transcript.includes('Stopped the stale reading; read all again for the current Face.')),
      `${label} has an unexplained spoken turn outcome ${turn.outcome}`);
      assert.ok(turn.completed_segments > 0);
      assert.ok(plays.some(played => played.face_id === turn.face_id &&
        played.face_revision_decimal === turn.face_revision_decimal &&
        played.source_show_id === turn.source_show_id),
      `${label} spoken turn is not correlated with its current Face and Show`);
    }
    const last = turns.at(-1);
    assert.equal(last.outcome, 'Completed');
    assert.equal(last.face_id, face.identity);
    assert.equal(last.face_revision_decimal, revision);
    return { first: { face_id: turns[0].face_id,
      face_revision: turns[0].face_revision_decimal,
      source_show_id: turns[0].source_show_id },
    final: { outcome: last.outcome, face_id: last.face_id,
      face_revision: last.face_revision_decimal, source_show_id: last.source_show_id,
      completed_segments: last.completed_segments, selected_playback_receipts: plays.length } };
  }
  assert.equal(turns.length, 0);
  assert.equal(plays.length, 0);
  const readouts = [...transcript.matchAll(/Text Face revision=(\d+) Show=(\S+)/g)];
  assert.ok(readouts.length > 0, `${label} produced no text readout`);
  const first = readouts[0], last = readouts.at(-1);
  assert.equal(last[1], revision);
  return { first: { face_revision: first[1], source_show_id: first[2] },
    final: { outcome: 'text-readout', face_id: face.identity,
      face_revision: last[1], source_show_id: last[2] } };
};
const waitFor = async (predicate, child, label, timeoutMillis = 15_000) => {
  const deadline = Date.now() + timeoutMillis;
  while (Date.now() < deadline) {
    const value = await predicate();
    if (value) return value;
    assert.equal(child.exitCode, null, `${label} exited before readiness`);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw new Error(`${label} did not become ready`);
};
const invoke = (executable, args, options = {}) => {
  const result = spawnSync(executable, args, {
    encoding: 'utf8', timeout: 120_000, maxBuffer: 8 * 1024 * 1024, ...options,
  });
  assert.equal(result.status, 0, `${args.slice(0, 3).join(' ')}: ${result.error ?? result.stderr}`);
  return result.stdout;
};
const ownerJson = args => JSON.parse(invoke(owner, args));
verifyGuestRouteCertificate(await readFile(cert), await readFile(key), routeUrl);
const installation = await load(path.join(state, 'installation.json'));
assert.equal(installation.product_executable, owner, 'installed owner executable differs');
const guestBuild = await load(path.join(build, 'build-manifest.json'));
const browserBundle = await load(path.join(handbook, 'sdk/bundle/conduit-browser-image.json'));
assert.equal(guestBuild.source_identity, installation.release_source_identity,
  'ConduitOS image must share the installed owner source before screen-free Birth');
assert.equal(browserBundle.reviewed_distribution.source_commit, installation.release_source_identity,
  'Handbook browser bundle must share the installed owner source before screen-free Birth');
if (speakerCard && model) {
  assert.equal(installation.selected_model?.model_name, model,
    'installed owner must select the listener model before screen-free Birth');
  assert.equal(installation.selected_speech?.card_id, speakerCard,
    'installed owner must select the listener speaker before screen-free Birth');
  assert.equal(installation.selected_speech?.device, Number(speakerDevice),
    'installed owner must select the listener device before screen-free Birth');
}
assert.equal(existsSync(path.join(state, 'body', 'biography.json')), false,
  'this producer requires an installed zero-Body Host');
assert.equal(existsSync(path.join(state, 'body', 'owner-transaction.json')), false,
  'pending Birth publication requires recovery, not a new Birth');
assert.equal(existsSync(output), false, 'output must be a new private directory');
await mkdir(output, { mode: 0o700 });
const ownerLog = path.join(output, 'owner-service.log');
const inviteFile = path.join(output, 'invitation.private.json');
let service, invitation, terminal, liveProof;
try {
  const serviceLog = openSync(ownerLog, 'wx', 0o600);
  service = spawn(owner, ['host', 'service', 'run', '--state-dir', state], {
    stdio: ['ignore', serviceLog, serviceLog],
  });
  closeSync(serviceLog);
  const before = await waitFor(() => {
    if (!existsSync(path.join(state, 'control.sock'))) return null;
    const status = spawnSync(owner, ['host', 'service', 'status', '--state-dir', state, '--json'],
      { encoding: 'utf8', timeout: 3000 });
    return status.status === 0 ? JSON.parse(status.stdout) : null;
  }, service, 'installed zero-Body service');
  const preBirth = makeZeroBodyReceipt(installation, before,
    existsSync(path.join(state, 'body', 'biography.json')),
    existsSync(path.join(state, 'body', 'owner-transaction.json')));
  const preBirthBytes = Buffer.from(`${JSON.stringify(preBirth, null, 2)}\n`);
  await writeFile(path.join(output, 'zero-body-before.json'), preBirthBytes, { mode: 0o600 });
  const birthCommands = [
    // Speech opens with Help and focused orientation. Explicitly request the
    // full zero-Body Face so this proof still exercises below-viewport reading.
    'read all',
    'next main',
    'focus creche.name', `edit value ${bodyName}`, 'activate',
    'focus creche.plot.0', 'edit value false', 'activate',
    'focus creche.plot.1', 'edit value true', 'activate',
    'review', 'focus creche.birth', 'activate',
    // Birth enters the retained Body with concise spoken orientation; request
    // the complete stable Face explicitly before leaving the Crèche chapter.
    'read all', 'quit',
  ];
  const input = `${birthCommands.join('\n')}\n`;
  const birthArgs = ['body', 'birth', '--screen-free', '--state-dir', state,
    ...selectedSpeechArgs];
  await writeFile(path.join(output, 'birth-input.txt'), input, { mode: 0o600 });
  const birthSession = speakerCard
    ? await runPacedScreenFree(owner, birthArgs, birthCommands, ['birth> ', 'body> '],
      screenFreeSessionTimeout) : null;
  if (birthSession) assert.deepEqual(birthSession.commands, birthCommands);
  const transcript = birthSession?.transcript ??
    invoke(owner, birthArgs, { input, timeout: screenFreeSessionTimeout });
  await writeFile(path.join(output, 'birth-transcript.txt'), transcript, { mode: 0o600 });
  for (const required of ['Installed Host screen-free Birth',
    'Body retained by this installed Host:', 'Continuing retained Body']) {
    assert.ok(transcript.includes(required), `Birth transcript lacks ${required}`);
  }
  if (!speakerCard) {
    for (const required of ['Edit Body name requested', 'Include Plot. For Clock',
      'Review Birth choices', 'Starting Plots selected: 1 of', 'Birth Body requested']) {
      assert.ok(transcript.includes(required), `Birth transcript lacks ${required}`);
    }
  }
  assert.ok(transcript.includes('Review Birth choices'),
    'Birth was not preceded by a current semantic review');
  const birthInteractions = [...transcript.matchAll(
    /Birth interaction: action=(\S+) face-id=(\S+) face-revision=(\d+) show=(\S+)/g)];
  assert.ok(birthInteractions.length >= 1, 'no actual Birth Face interaction was reported');
  const [birthAction, birthFaceId, birthFaceRevision, birthShowId] =
    birthInteractions.at(-1).slice(1);
  assert.equal(birthAction, 'creche.birth', 'the final Birth interaction was not explicit');
  assert.ok(birthFaceId && birthShowId);
  const born = ownerJson(['body', 'status', '--state-dir', state, '--json']);
  const bornFace = ownerJson(['body', 'face', '--state-dir', state, '--json']);
  const bodyId = born.biography.body_id;
  assert.ok(bodyId && transcript.includes(bodyId));
  assert.equal(bornFace.presentation.basis.body_id, bodyId);
  assert.equal(born.biography.membership.parts.length, 1);
  const ownerPart = born.biography.membership.parts[0].current;
  assert.ok(ownerPart?.host_id && ownerPart.boot_id);
  // The zero-Body observation and born owner are one installed Boot.
  const beforeText = JSON.stringify(before);
  assert.ok(beforeText.includes(ownerPart.host_id) && beforeText.includes(ownerPart.boot_id));
  assert.match(await readFile(path.join(state, 'body', 'source.conduit'), 'utf8'),
    /time\/every\(1s\)/);
  const finalReading = attestReading(transcript, bornFace, ownerPart,
    'screen-free Birth').final;
  const birth = {
    proof_class: speakerCard ? 'installed-screen-free-birth-selected-alsa' :
      'installed-screen-free-birth-text-readout',
    body_id: bodyId,
    owner_host_id: ownerPart.host_id,
    owner_boot_id: ownerPart.boot_id,
    source_commit: installation.release_source_identity,
    zero_body_observed: true,
    zero_body_receipt: { path: '../zero-body-before.json',
      bytes: preBirthBytes.length, sha256: digest(preBirthBytes) },
    confirmation_observed: true,
    review_requested: true,
    birth_interaction: { action_id: birthAction, face_id: birthFaceId,
      face_revision: birthFaceRevision, show_id: birthShowId },
    speaker_playback_selected: Boolean(speakerCard),
    human_hearing_observed: false,
    owner_instance_speech_realization_observed: false,
    final_reading: finalReading,
    input: { path: '../birth-input.txt', sha256: digest(Buffer.from(input)) },
    transcript: { path: '../birth-transcript.txt', sha256: digest(Buffer.from(transcript)) },
  };
  const audioChapters = [];
  const audioSessions = [];
  if (birthSession) {
    const response = index => {
      assert.equal(birthSession.responses[index]?.command, birthCommands[index]);
      return birthSession.responses[index].output;
    };
    const reviewIndex = birthCommands.indexOf('review');
    const birthIndex = birthCommands.indexOf('focus creche.birth') + 1;
    assert.equal(birthCommands[birthIndex], 'activate');
    audioChapters.push(
      { name: 'zero-body-orientation', output: response(0) },
      { name: 'birth-review', output: response(reviewIndex), includeLast: true },
      { name: 'explicit-birth-result', output: response(birthIndex),
        face_id: bornFace.presentation.identity,
        face_revision: bornFace.presentation_revision_decimal, includeLast: true },
    );
    audioSessions.push({ name: 'birth', transcript });
  }
  const inviteFd = openSync(inviteFile, 'wx', 0o600);
  const inviteLog = openSync(path.join(output, 'invitation-service.log'), 'wx', 0o600);
  invitation = spawn(owner, ['body', 'invite', '--state-dir', state, '--ttl-seconds', '600',
    '--route-bind', forward, '--route-url', routeUrl,
    '--route-tls-cert', cert, '--route-tls-key', key, '--authorize-route'],
  { stdio: ['ignore', inviteFd, inviteLog] });
  closeSync(inviteFd);
  closeSync(inviteLog);
  const invite = await waitFor(async () => {
    if (!existsSync(inviteFile)) return null;
    try { return await load(inviteFile); } catch { return null; }
  }, invitation, 'private owner invitation');
  assert.equal(invite.claim.body_id, bodyId);
  const candidateId = invite.rendezvous.candidates[0].candidate_id;
  assert.ok(candidateId);
  const authored = `body shared {\n  schema = 2\n  id = ${JSON.stringify(bodyId)}\n  host = {name: "native", configuration: ${JSON.stringify(profile)}, spore: {join_mode: "self-joining", invitation: ${JSON.stringify(invite.claim.invitation_id)}, output: "disk-image"}}\n}\n`;
  const bodyFile = path.join(output, 'shared.body.conduit');
  await writeFile(bodyFile, authored, { mode: 0o600 });
  const spore = path.join(output, 'native.private.iso');
  invoke(xtask, ['make', 'body', 'provision-conduitos', bodyFile, '--host', 'native',
    '--build', build, '--invitation', inviteFile, '--route-tls-cert', cert, '--output', spore],
  { timeout: 120_000 });
  const live = path.join(output, 'three-host');
  const liveArgs = ['make', 'conduitos', 'live-three-host-proof', '--owner', owner,
    '--owner-state', state, '--handbook', handbook, '--spore', spore,
    '--candidate-id', candidateId, '--owner-forward', forward,
    '--output-dir', live, '--playwright', playwright];
  if (speechExecutable) liveArgs.push('--speech-executable', speechExecutable,
    '--speech-data', speechData, '--speech-engine', speechEngine,
    '--speech-language-coverage', speechLanguageCoverage);
  if (model) liveArgs.push('--model', model, '--ollama-endpoint', modelEndpoint,
    '--admitted-memory-mib', modelMemory);
  if (ownerModelRouteControlArg && ownerModelRouteControlArg !== '-') {
    liveArgs.push('--owner-model-route-control', path.resolve(ownerModelRouteControlArg));
  }
  // The owner Journey pauses at each environmental loss/return. The person
  // enters the public nonvisual Mask while the underlying route state is held,
  // and the owner resumes only after a current Face, wardrobe, and Play receipt.
  const checkpointDir = path.join(live, 'screen-free-checkpoints');
  const phases = checkpointPhases.filter(phase =>
    !phase.startsWith('model-') ||
    (model && ownerModelRouteControlArg && ownerModelRouteControlArg !== '-'));
  const checkpoints = [];
  const checkpointSessions = [];
  const checkpointLog = path.join(output, 'three-host-proof.log');
  const proofLog = openSync(checkpointLog, 'wx', 0o600);
  liveProof = spawn(xtask, liveArgs, { stdio: ['ignore', proofLog, proofLog],
    env: { ...process.env, CONDUIT_SCREEN_FREE_CHECKPOINTS: '1' } });
  closeSync(proofLog);
  let liveError;
  liveProof.once('error', error => { liveError = error; });
  const liveDeadline = Date.now() + (speakerCard ? 55 * 60_000 : 10 * 60_000);
  for (const phase of phases) {
    const readyFile = path.join(checkpointDir, `${phase}.ready.json`);
    const ready = await waitFor(async () => {
      if (!existsSync(readyFile)) return null;
      try { return await load(readyFile); } catch { return null; }
    }, liveProof, `${phase} checkpoint`, Math.max(1, liveDeadline - Date.now()));
    const current = ownerJson(['body', 'face', '--state-dir', state, '--json']);
    verifyCheckpointReady(ready, phase, bodyId, ownerPart, current);
    const commands = ['wardrobe', 'read all', 'quit'];
    const args = ['body', 'screen-free', '--state-dir', state, ...selectedSpeechArgs];
    const session = await runPacedScreenFree(owner, args, commands, 'body> ',
      Math.min(screenFreeSessionTimeout, liveDeadline - Date.now()),
      { retryStaleReadAll: 4 });
    assert.equal(session.responses[0]?.command, 'wardrobe');
    const wardrobeReading = verifyCheckpointWardrobe(session.responses[0].output,
      ready, ownerPart, Boolean(speakerCard));
    const readAll = session.responses.findLast(item => item.command === 'read all');
    assert.ok(readAll, `${phase} lacks the operator's whole-Face request`);
    const after = ownerJson(['body', 'face', '--state-dir', state, '--json']);
    assert.equal(after.presentation.basis.body_id, bodyId);
    assert.equal(after.advertisement.host_id, ownerPart.host_id);
    assert.equal(after.advertisement.boot_id, ownerPart.boot_id);
    assert.ok(BigInt(after.presentation_revision_decimal) >=
      BigInt(current.presentation_revision_decimal));
    const finalReading = attestReading(readAll.output, after, ownerPart,
      `screen-free checkpoint ${phase}`, true).final;
    const input = `${session.commands.join('\n')}\n`;
    const inputName = `screen-free-checkpoint-${phase}-input.txt`;
    const transcriptName = `screen-free-checkpoint-${phase}-transcript.txt`;
    await writeFile(path.join(live, inputName), input, { flag: 'wx', mode: 0o600 });
    await writeFile(path.join(live, transcriptName), session.transcript,
      { flag: 'wx', mode: 0o600 });
    const transcriptBytes = Buffer.from(session.transcript);
    const resume = {
      schema: 'conduit.proof/screen-free-checkpoint@1', phase,
      body_id: bodyId, owner_host_id: ownerPart.host_id, owner_boot_id: ownerPart.boot_id,
      route_id: ready.route_id, route_available: ready.route_available,
      wardrobe_revision: wardrobeReading.wardrobe_revision,
      face_id: after.presentation.identity,
      face_revision: after.presentation_revision_decimal,
      source_show_id: finalReading.source_show_id,
      speaker_playback_selected: Boolean(speakerCard),
      selected_playback_receipts: finalReading.selected_playback_receipts ?? 0,
      transcript: { path: `../${transcriptName}`, sha256: digest(transcriptBytes),
        bytes: transcriptBytes.length },
    };
    const resumeFile = path.join(checkpointDir, `${phase}.resume.json`);
    const temporaryResume = `${resumeFile}.tmp`;
    await writeFile(temporaryResume, `${JSON.stringify(resume, null, 2)}\n`,
      { flag: 'wx', mode: 0o600 });
    await rename(temporaryResume, resumeFile);
    checkpoints.push({ phase, ready: { path: `screen-free-checkpoints/${phase}.ready.json`,
      sha256: digest(await readFile(readyFile)) },
    resume: { path: `screen-free-checkpoints/${phase}.resume.json`,
      sha256: digest(await readFile(resumeFile)) },
    ...wardrobeReading, final_reading: finalReading,
    input: { path: inputName, bytes: Buffer.byteLength(input),
      sha256: digest(Buffer.from(input)) },
    transcript: { path: transcriptName, sha256: resume.transcript.sha256,
      bytes: resume.transcript.bytes } });
    if (speakerCard) {
      audioChapters.push({ name: `checkpoint-${phase}`, output: readAll.output,
        face_id: after.presentation.identity,
        face_revision: after.presentation_revision_decimal });
      checkpointSessions.push({ name: `checkpoint-${phase}`,
        transcript: session.transcript });
    }
  }
  const done = await waitFor(() => liveProof.exitCode !== null || liveProof.signalCode !== null,
    { exitCode: null }, 'three-host proof completion',
    Math.max(1, liveDeadline - Date.now()));
  assert.ok(done);
  assert.equal(liveError, undefined, String(liveError));
  assert.equal(liveProof.exitCode, 0,
    `three-host proof failed: ${(await readFile(checkpointLog, 'utf8')).slice(-3000)}`);
  const reportFile = path.join(live, 'report.json');
  const report = await load(reportFile);
  assert.equal(report.body_id, bodyId);
  assert.equal(report.owner_host_id, ownerPart.host_id);
  assert.equal(report.owner_boot_id, ownerPart.boot_id);
  assert.equal(report.native_source_commit, installation.release_source_identity);
  report.birth = birth;
  report.screen_free_checkpoints = {
    proof_class: speakerCard ? 'installed-screen-free-loss-recovery-selected-alsa' :
      'installed-screen-free-loss-recovery-text-readout',
    run_id: report.run_id, body_id: bodyId,
    owner_host_id: ownerPart.host_id, owner_boot_id: ownerPart.boot_id,
    source_commit: installation.release_source_identity,
    environmental_changes_performed_by: 'three-host owner Journey, not screen-free commands',
    speaker_playback_selected: Boolean(speakerCard),
    human_hearing_observed: false, checkpoints,
  };
  audioSessions.push(...checkpointSessions);
  const beforeStart = ownerJson(['body', 'face', '--state-dir', state, '--json']);
  assert.equal(beforeStart.presentation.basis.body_id, bodyId);
  assert.equal(beforeStart.advertisement.host_id, ownerPart.host_id);
  assert.equal(beforeStart.advertisement.boot_id, ownerPart.boot_id);
  const available = (face, intent) => face.presentation.actions.find(action =>
    action.intent === intent && action.availability === 'Available');
  const clockAudioSessions = [];
  const exercise = async (name, before, action) => {
    assert.ok(action, `the current owner Face offers no available ${name} action`);
    const commands = [...(speakerCard ? [] : ['read all']),
      `focus ${action.identity}`, 'activate',
      ...(speakerCard && name === 'start' ? [] : ['read all']), 'quit'];
    const input = `${commands.join('\n')}\n`;
    const inputFile = `clock-${name}-input.txt`;
    const transcriptFile = `clock-${name}-transcript.txt`;
    await writeFile(path.join(output, inputFile), input, { mode: 0o600 });
    const args = ['body', 'screen-free', '--state-dir', state, ...selectedSpeechArgs];
    const selected = speakerCard
      ? await runPacedScreenFree(owner, args, commands, 'body> ', screenFreeSessionTimeout,
        { retryStaleReadAll: name === 'start' ? 0 : 4 }) : null;
    const actualInput = selected ? `${selected.commands.join('\n')}\n` : input;
    if (selected) await writeFile(path.join(output, inputFile), actualInput, { mode: 0o600 });
    const transcript = selected?.transcript ??
      invoke(owner, args, { input, timeout: screenFreeSessionTimeout });
    await writeFile(path.join(output, transcriptFile), transcript, { mode: 0o600 });
    assert.ok(transcript.includes(`Continuing retained Body ${bodyId}`));
    const enacted = [...transcript.matchAll(/Interaction: action=(\S+) face-revision=(\d+) show=(\S+)/g)];
    assert.equal(enacted.length, 1, `screen-free ${name} must submit one semantic action`);
    assert.equal(enacted[0][1], action.identity);
    assert.equal(enacted[0][2], before.presentation_revision_decimal);
    assert.match(transcript, /Owner action result:/);
    const after = ownerJson(['body', 'face', '--state-dir', state, '--json']);
    assert.equal(after.presentation.basis.body_id, bodyId);
    assert.ok(BigInt(after.presentation_revision_decimal) >
      BigInt(before.presentation_revision_decimal));
    const reading = attestReading(transcript, after, ownerPart,
      `screen-free clock ${name}`, true);
    assert.equal(reading.first.face_revision, before.presentation_revision_decimal);
    assert.equal(enacted[0][3], reading.first.source_show_id);
    if (selected) {
      const lastReading = selected.responses.findLast(response => response.command === 'read all');
      const actionResponse = selected.responses.find(response => response.command === 'activate');
      audioChapters.push({ name: `clock-${name}`,
        output: (lastReading ?? actionResponse).output,
        face_id: after.presentation.identity,
        face_revision: after.presentation_revision_decimal });
      clockAudioSessions.push({ name: `clock-${name}`, transcript });
    }
    return {
      action_id: action.identity, source_face_id: before.presentation.identity,
      source_face_revision: before.presentation_revision_decimal, source_show_id: enacted[0][3],
      result_face_id: after.presentation.identity,
      result_face_revision: after.presentation_revision_decimal,
      final_reading: reading.final,
      input: { path: `../${inputFile}`, bytes: Buffer.byteLength(actualInput),
        sha256: digest(Buffer.from(actualInput)) },
      transcript: { path: `../${transcriptFile}`, bytes: Buffer.byteLength(transcript),
        sha256: digest(Buffer.from(transcript)) },
      after,
    };
  };
  const start = await exercise('start', beforeStart,
    available(beforeStart, 'conduit.intent/start-clock@1'));
  const afterStart = ownerJson(['body', 'status', '--state-dir', state, '--json']);
  assert.equal(afterStart.biography.body_id, bodyId);
  assert.ok(afterStart.biography.body.state.Awake);
  assert.ok(available(start.after, 'conduit.intent/lull-clock@1'));
  const lull = await exercise('lull', start.after,
    available(start.after, 'conduit.intent/lull-clock@1'));
  const afterLull = ownerJson(['body', 'status', '--state-dir', state, '--json']);
  assert.equal(afterLull.biography.body_id, bodyId);
  assert.equal(afterLull.biography.body.state, 'Lulled');
  assert.equal(afterLull.biography.membership.parts[0].current.host_id, ownerPart.host_id);
  assert.equal(afterLull.biography.membership.parts[0].current.boot_id, ownerPart.boot_id);
  // The terminal route is offered only while a real installed terminal Mask
  // provider is attached. Keep that provider open through the nonvisual
  // inspect/refusal/recovery/doff/wear/prefer sequence.
  let terminalOutput = '';
  let terminalError = '';
  terminal = spawn(owner, ['body', 'terminal', '--state-dir', state, '--owner-show'],
    { stdio: ['pipe', 'pipe', 'pipe'] });
  terminal.stdout.on('data', chunk => { terminalOutput += chunk.toString('utf8'); });
  terminal.stderr.on('data', chunk => { terminalError += chunk.toString('utf8'); });
  await waitFor(() => terminalOutput.includes('browser selection awaits a complete carrier-Line Mask Plan.'),
    terminal, 'attached owner terminal Mask');
  assert.match(terminalOutput, /Owner terminal Show/);
  const wardrobeArgs = ['body', 'screen-free', '--state-dir', state, ...selectedSpeechArgs];
  const wardrobeSession = await runPacedScreenFree(owner, wardrobeArgs,
    screenFreeWardrobeCommands, 'body> ', screenFreeSessionTimeout);
  const wardrobe = verifyScreenFreeWardrobe(wardrobeSession, ownerPart, Boolean(speakerCard));
  if (speakerCard) {
    audioChapters.push(
      { name: 'wardrobe-inspection', output: wardrobeSession.responses[0].output },
      { name: 'wardrobe-recovery', output: wardrobeSession.responses[2].output },
      { name: 'wardrobe-preference', output: wardrobeSession.responses[5].output },
    );
    audioSessions.push({ name: 'wardrobe', transcript: wardrobeSession.transcript });
  }
  const wardrobeInput = `${wardrobeSession.commands.join('\n')}\n`;
  await writeFile(path.join(output, 'wardrobe-input.txt'), wardrobeInput, { mode: 0o600 });
  await writeFile(path.join(output, 'wardrobe-transcript.txt'), wardrobeSession.transcript,
    { mode: 0o600 });
  const terminalExit = new Promise(resolve => terminal.once('close', resolve));
  terminal.stdin.end('quit\n');
  const terminalCode = await Promise.race([
    terminalExit,
    new Promise((_, reject) => setTimeout(() => reject(new Error('owner terminal detachment timed out')), 10_000)),
  ]);
  assert.equal(terminalCode, 0, `owner terminal detachment: ${terminalError}`);
  await writeFile(path.join(output, 'wardrobe-provider-transcript.txt'), terminalOutput,
    { mode: 0o600 });
  const afterWardrobe = ownerJson(['body', 'status', '--state-dir', state, '--json']);
  assert.equal(afterWardrobe.biography.body_id, bodyId);
  assert.equal(afterWardrobe.biography.body.state, 'Lulled');
  const beforeWake = ownerJson(['body', 'face', '--state-dir', state, '--json']);
  const wake = await exercise('wake', beforeWake,
    available(beforeWake, 'conduit.intent/start-clock@1'));
  const afterWake = ownerJson(['body', 'status', '--state-dir', state, '--json']);
  assert.equal(afterWake.biography.body_id, bodyId);
  assert.ok(afterWake.biography.body.state.Awake);
  const finish = await exercise('finish', wake.after,
    available(wake.after, 'conduit.intent/lull-clock@1'));
  const afterFinish = ownerJson(['body', 'status', '--state-dir', state, '--json']);
  assert.equal(afterFinish.biography.body_id, bodyId);
  assert.equal(afterFinish.biography.body.state, 'Lulled');
  assert.equal(afterFinish.biography.membership.parts[0].current.host_id, ownerPart.host_id);
  assert.equal(afterFinish.biography.membership.parts[0].current.boot_id, ownerPart.boot_id);
  delete start.after;
  delete lull.after;
  delete wake.after;
  delete finish.after;
  report.screen_free_clock = {
    proof_class: speakerCard ? 'installed-screen-free-clock-selected-alsa' :
      'installed-screen-free-clock-text-readout',
    run_id: report.run_id, body_id: bodyId,
    owner_host_id: ownerPart.host_id, owner_boot_id: ownerPart.boot_id,
    source_commit: installation.release_source_identity,
    start, lull, wake, finish,
    speaker_playback_selected: Boolean(speakerCard), human_hearing_observed: false,
  };
  report.screen_free_wardrobe = {
    proof_class: speakerCard ? 'installed-screen-free-wardrobe-selected-alsa' :
      'installed-screen-free-wardrobe-text-readout',
    run_id: report.run_id, body_id: bodyId,
    owner_host_id: ownerPart.host_id, owner_boot_id: ownerPart.boot_id,
    source_commit: installation.release_source_identity,
    ...wardrobe,
    provider_transcript: { path: '../wardrobe-provider-transcript.txt',
      bytes: Buffer.byteLength(terminalOutput), sha256: digest(Buffer.from(terminalOutput)) },
    input: { path: '../wardrobe-input.txt', bytes: Buffer.byteLength(wardrobeInput),
      sha256: digest(Buffer.from(wardrobeInput)) },
    transcript: { path: '../wardrobe-transcript.txt',
      bytes: Buffer.byteLength(wardrobeSession.transcript),
      sha256: digest(Buffer.from(wardrobeSession.transcript)) },
    speaker_playback_selected: Boolean(speakerCard), human_hearing_observed: false,
  };
  if (speakerCard) {
    report.screen_free_audio = await retainSelectedScreenFreePlays({
      state, privateRoot: output, walkthroughRoot: live, ownerPart, installation,
      chapters: audioChapters,
      sessions: [...audioSessions, ...clockAudioSessions],
    });
  }
  await retainScreenFreeSessions(output, live, report);
  // Publication is a different boundary from the diagnostic walkthrough. Build
  // this index only after every producer has finished, from the identities and
  // captures observed in this run. A missing transition is a refusal, never an
  // invitation to fill a chapter from prose or a previous run.
  if (speakerCard && model && ownerModelRouteControlArg && ownerModelRouteControlArg !== '-') {
    const exact = (value, label) => {
      assert.ok(value !== undefined && value !== null && value !== '', `${label} is missing`);
      return value;
    };
    const observations = new Map(report.observations.map(asyncReceipt =>
      [path.basename(asyncReceipt.path, '.json'), asyncReceipt]));
    const observation = async name => {
      const artifact = exact(observations.get(name), `${name} capture observation`);
      const bytes = await readFile(path.join(live, artifact.path));
      assert.equal(digest(bytes), artifact.sha256);
      return JSON.parse(bytes);
    };
    const screenshot = file => exact(report.screenshots.find(item => item.path === file), file);
    const checkpoint = phase => exact(checkpoints.find(item => item.phase === phase), phase);
    const publicationDirectory = path.join(live, 'publication-receipts');
    await mkdir(publicationDirectory, { mode: 0o700 });
    let receiptNumber = 0;
    const emit = async (type, record) => {
      const file = `publication-receipts/${String(++receiptNumber).padStart(2, '0')}-${type}.json`;
      const bytes = Buffer.from(`${JSON.stringify({
        schema: `conduit.proof/three-host-publication-${type}@1`,
        source_commit: report.native_source_commit, run_id: report.run_id,
        body_id: report.body_id, ...record,
      }, null, 2)}\n`);
      await writeFile(path.join(live, file), bytes, { flag: 'wx', mode: 0o600 });
      return { path: file, sha256: digest(bytes) };
    };
    const event = async (kind, id, faceRevision, basis) => {
      exact(kind, 'event kind'); exact(id, 'event identity');
      exact(faceRevision, 'event Face revision'); exact(basis, 'event basis');
      const sourceReceipt = await emit('event', {
        event_kind: kind, event_id: id,
        resulting_face_revision: String(faceRevision), outcome: 'completed',
        observed_basis_sha256: digest(Buffer.from(JSON.stringify(basis))),
      });
      return { kind, id, face_revision: String(faceRevision), source_receipt: sourceReceipt };
    };
    const media = async (captureSource, artifact, eventRecord, alt, basis, speech) => {
      exact(artifact?.path, 'media path'); exact(artifact?.sha256, 'media digest');
      const bytes = await readFile(path.join(live, artifact.path));
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
    const nativeReceipt = await load(path.join(live, 'native/owner-action-proof.json'));
    assert.equal(nativeReceipt.source_commit, report.native_source_commit);
    assert.equal(nativeReceipt.guest_part.part_id, report.guest_part_id);
    assert.equal(nativeReceipt.action.action_id, report.native_action.action_id);
    const recovery = await load(path.join(live, report.presentation_host_recovery.path));
    assert.equal(recovery.body_id, report.body_id);
    const loss = exact(report.owner_model_route_loss, 'selected model route loss');
    const direct = exact(report.owner_direct_speech, 'direct owner speech');
    const llm = exact(report.owner_llm_speech, 'selected owner model speech');
    const originalModelOutput = (await load(path.join(live, llm.terminal.path)))
      .generation_evidence.original_model_output;
    const restoredModelOutput = (await load(path.join(live, loss.restored.terminal.path)))
      .generation_evidence.original_model_output;
    assert.equal(digest(Buffer.from(originalModelOutput)), llm.original_model_output_sha256);
    assert.equal(digest(Buffer.from(restoredModelOutput)),
      loss.restored.original_model_output_sha256);
    const born = await event('typed-interaction', birth.birth_interaction.action_id,
      birth.final_reading.face_revision, birth.birth_interaction);
    const joinedBrowser = await event('membership', report.browser_part_id,
      browserJoin.resulting_face.face_revision, browserJoin);
    const joinedGuest = await event('membership', report.guest_part_id,
      nativeReceipt.face_before.face_revision, nativeReceipt.guest_part);
    const started = await event('typed-interaction', start.action_id,
      start.result_face_revision, start);
    const changedNative = await event('typed-interaction', nativeReceipt.action.interaction_id,
      nativeReceipt.face_after.face_revision, nativeReceipt.action);
    // The browser UI exposes the typed action and exact source Show, but does
    // not expose an invocation ID. Bind this accepted return to its real Show
    // instead of pretending the reusable action identity is a unique event.
    const changedBrowser = await event('typed-interaction-source-show',
      browserAction.cause.source_show_id,
      browserAction.resulting_face.face_revision, browserAction.cause);
    const changedTerminal = await event('typed-interaction', report.terminal_action.interaction_id,
      browserTerminal.resulting_face.face_revision, browserTerminal.cause);
    const selectedWardrobe = await event('acknowledged-show', report.browser_wardrobe.selected_show_id,
      browserWardrobe.resulting_face.face_revision, report.browser_wardrobe);
    const heardDirect = await event('selected-speaker-play', direct.batches[0].play_id,
      direct.face_revision_decimal, direct.batches[0]);
    const heardModel = await event('selected-speaker-play', llm.listener_play_id,
      llm.face_revision_decimal, llm);
    const modelLost = await event('provider-withdrawal', `route/${loss.route_plan_id}`,
      checkpoint('model-provider-unavailable').final_reading.face_revision,
      checkpoint('model-provider-unavailable'));
    const browserLost = await event('presentation-leave', recovery.lost_browser_boot_id,
      checkpoint('browser-presentation-unavailable').final_reading.face_revision,
      recovery.loss);
    const modelReturned = await event('selected-speaker-play', loss.restored.listener_play_id,
      loss.restored.face_revision_decimal, loss.restored);
    const browserReturned = await event('membership-return', recovery.recovered_browser_boot_id,
      browserRecovery.resulting_face.face_revision, recovery);
    const lulled = await event('typed-interaction', finish.action_id,
      finish.result_face_revision, finish);
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
    const directBatch = direct.batches[0];
    const directSpeech = { mode: 'direct', show_id: direct.show_id,
      plan_id: directBatch.plan_id, play_id: directBatch.play_id,
      voice_id: installation.selected_speech.voice,
      text: directBatch.spoken_segments.join(' '), provenance: direct.terminal };
    const modelSpeech = (record, originalOutput) => ({ mode: 'llm-assisted', show_id: record.show_id,
      plan_id: record.listener_plan_id, play_id: record.listener_play_id,
      voice_id: installation.selected_speech.voice,
      text: record.accepted_wording, provenance: record.terminal,
      original_model_output: originalOutput,
      provider_id: record.provider_identity,
      model_id: record.model_identity });
    report.publication_chapters = [
      chapter('birth', 'Make a Body', 'Create one Body without a screen.',
        'Review the clock Plot and activate Birth.', 'The installed owner retains the new Body.',
        'The other Hosts must join this same Body.', 'Open the browser and QEMU guest.',
        ['No attended human listening is claimed.'], [born],
        [await transcriptMedia(birth.transcript, born, 'Actual nonvisual Birth session')]),
      chapter('join', 'Give it more places to meet you', 'Meet the same Body on three Hosts.',
        'Join through the browser and QEMU guest.', 'Three current Parts have distinct Host and Boot identities.',
        'Membership does not silently birth another Body.', 'Start the clock.',
        ['QMP is emulator evidence, not physical hardware evidence.'],
        [joinedBrowser, joinedGuest], [
          await chromium('browser-before.png', joinedBrowser, 'Joined browser Face', browserJoin),
          await qmp('native/owner-standby.png', joinedGuest, 'Joined QEMU guest before Mask activation', nativeReceipt.face_before),
        ]),
      chapter('start', 'Start something useful', 'Run a clock and change its interval.',
        'Start nonvisually, then set the interval on ConduitOS and in the browser.',
        'Typed interactions change the shared owner Face.',
        'The workload remains on its admitted owner.', 'Inspect another Mask.',
        ['This does not prove distributed workload migration.'],
        [started, changedNative, changedBrowser], [
          await transcriptMedia(start.transcript, started, 'Nonvisual clock Start session'),
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
          await chromium('browser-after-terminal.png', changedTerminal, 'Browser Face after terminal action', browserTerminal),
          await chromium('browser-wardrobe.png', selectedWardrobe, 'Browser wardrobe after preference change', browserWardrobe),
        ]),
      chapter('hear', 'Hear it', 'Listen to the current Face and its explanation.',
        'Select direct and model-assisted speech.',
        'Both selected speaker Plays complete with retained listener WAVs.',
        'The recording is from the same Play as the speaker output.', 'Withdraw a route.',
        ['Automated playback does not establish attended human hearing.'],
        [heardDirect, heardModel], [
          await media('selected-speaker-same-play', directBatch.wav, heardDirect,
            'Direct spoken Face from the selected speaker Play', directBatch, directSpeech),
          await media('selected-speaker-same-play', llm.wav, heardModel,
            'Model-assisted explanation from the selected speaker Play', llm,
            modelSpeech(llm, originalModelOutput)),
        ]),
      chapter('loss', 'Change the circumstances', 'Understand what fails when routes disappear.',
        'Withdraw the model route and leave the browser presentation.',
        'Model Start refuses before a new Play; the old browser actions become unavailable.',
        'Availability and a current Show are separate from Body identity.', 'Return on fresh routes.',
        ['No automatic fallback or workload failover is claimed.'],
        [modelLost, browserLost], [
          await media('terminal', loss.refusal, modelLost,
            'Owner refusal when the selected model route is withdrawn', loss),
          await transcriptMedia(checkpoint('model-provider-unavailable').transcript,
            modelLost, 'Screen-free inspection during model route loss'),
          await transcriptMedia(checkpoint('browser-presentation-unavailable').transcript,
            browserLost, 'Screen-free inspection during browser presentation loss'),
        ]),
      chapter('return', 'Come back', 'Return to the retained Body.',
        'Restore the model route and rejoin the browser on a fresh Boot.',
        'Fresh selected speaker Play and browser Show replace stale facts.',
        'The owner preserves Body continuity while route identities change.', 'Lull the clock.',
        ['No QEMU reboot is claimed.'], [modelReturned, browserReturned], [
          await media('selected-speaker-same-play', loss.restored.wav, modelReturned,
            'Restored model explanation from the selected speaker Play',
            loss.restored, modelSpeech(loss.restored, restoredModelOutput)),
          await chromium('browser-after-recovery.png', browserReturned,
            'Fresh browser Boot showing the same Body', browserRecovery),
        ]),
      chapter('lull', 'Leave it well', 'Leave the clock in a known retained state.',
        'Activate Lull nonvisually and read the resulting Face.',
        'The clock Play retires while the Body remains retained.',
        'Retained identity is distinct from continuing execution.', 'Inspect the report.',
        ['A retained Body is not proof of an active Play.'], [lulled],
        [await transcriptMedia(finish.transcript, lulled, 'Nonvisual Lull session')]),
    ];
  }
  const walkthrough = await writeThreeHostWalkthrough(live, handbook, report);
  report.walkthrough = {
    ...walkthrough,
    sha256: digest(await readFile(path.join(live, walkthrough.path))),
    assets: await Promise.all(['conduit.css', 'chrome.css'].map(async file => ({
      path: file, sha256: digest(await readFile(path.join(live, file))),
    }))),
  };
  await writeFile(reportFile, `${JSON.stringify(report, null, 2)}\n`);
  await verifyWalkthroughAssets(live, await readFile(path.join(live, walkthrough.path), 'utf8'));
  console.log(`Screen-free Birth and three-host proof: ${reportFile}`);
} finally {
  if (liveProof?.exitCode === null) liveProof.kill();
  if (terminal?.exitCode === null) terminal.kill();
  if (invitation?.exitCode === null) invitation.kill();
  if (service?.exitCode === null) service.kill();
}
