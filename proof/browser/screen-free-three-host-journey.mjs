// Producer-owned journey: an installed zero-Body Host receives real nonvisual
// Birth commands before its one Body is provisioned for the three-host proof.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, existsSync, openSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { writeThreeHostWalkthrough } from './three-host-walkthrough.mjs';
import { makeZeroBodyReceipt } from './zero-body-receipt.mjs';
import { runPacedScreenFree } from './paced-screen-free-input.mjs';

const [xtaskArg, ownerArg, stateArg, handbookArg, buildArg, profileArg,
  certArg, keyArg, forward, routeUrl, outputArg, playwrightArg, bodyName,
  speakerCardArg, speakerDeviceArg, speechExecutableArg, speechDataArg, speechEngineArg,
  modelArg, modelEndpoint, modelMemory] = process.argv.slice(2);
const [speakerCard, speakerDevice, speechExecutable, speechData, speechEngine] =
  [speakerCardArg, speakerDeviceArg, speechExecutableArg, speechDataArg, speechEngineArg]
    .map(value => value === '-' ? undefined : value);
const model = modelArg === '-' ? undefined : modelArg;
assert.ok(bodyName && !bodyName.includes('\n') && !bodyName.includes('\r') &&
  Buffer.byteLength(bodyName) <= 120, 'Body name must be one bounded input line');
assert.equal(Boolean(speakerCard), Boolean(speakerDevice));
assert.ok(!speakerCard || speechExecutable, 'selected speaker needs a speech provider');
assert.equal(Boolean(speechExecutable), Boolean(speechData));
assert.equal(Boolean(speechExecutable), Boolean(speechEngine));
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
  '--speech-data', speechData, '--speech-engine', speechEngine] : [];
// Full-Face playback is serialized at the selected ALSA device. The scripted
// Birth and clock sessions each include multiple complete readings, not a
// single generated artifact; retain a finite wall-clock deadline for them.
const screenFreeSessionTimeout = speakerCard ? 30 * 60_000 : 30_000;
const attestReading = (transcript, face, part, label, allowStaleCancellation = false) => {
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
        played.source_show_id === turn.source_show_id),
      `${label} spoken turn is not correlated with its current Face and Show`);
    }
    const last = turns.at(-1);
    assert.equal(last.outcome, 'Completed');
    assert.equal(last.face_id, face.identity);
    assert.equal(last.face_revision, face.revision);
    return { first: { face_id: turns[0].face_id, face_revision: turns[0].face_revision,
      source_show_id: turns[0].source_show_id },
    final: { outcome: last.outcome, face_id: last.face_id,
      face_revision: last.face_revision, source_show_id: last.source_show_id,
      completed_segments: last.completed_segments, selected_playback_receipts: plays.length } };
  }
  assert.equal(turns.length, 0);
  assert.equal(plays.length, 0);
  const readouts = [...transcript.matchAll(/Text Face revision=(\d+) Show=(\S+)/g)];
  assert.ok(readouts.length > 0, `${label} produced no text readout`);
  const first = readouts[0], last = readouts.at(-1);
  assert.equal(Number(last[1]), face.revision);
  return { first: { face_revision: Number(first[1]), source_show_id: first[2] },
    final: { outcome: 'text-readout', face_id: face.identity,
      face_revision: Number(last[1]), source_show_id: last[2] } };
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
const installation = await load(path.join(state, 'installation.json'));
assert.equal(installation.product_executable, owner, 'installed owner executable differs');
assert.equal(existsSync(path.join(state, 'body', 'biography.json')), false,
  'this producer requires an installed zero-Body Host');
assert.equal(existsSync(path.join(state, 'body', 'owner-transaction.json')), false,
  'pending Birth publication requires recovery, not a new Birth');
assert.equal(existsSync(output), false, 'output must be a new private directory');
await mkdir(output, { mode: 0o700 });
const ownerLog = path.join(output, 'owner-service.log');
const inviteFile = path.join(output, 'invitation.private.json');
let service, invitation;
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
    // Selected speech already opens with Help and a complete Face reading.
    ...(speakerCard ? [] : ['read all']),
    'next main',
    'focus creche.name', `edit value ${bodyName}`, 'activate',
    'focus creche.plot.0', 'edit value false', 'activate',
    'focus creche.plot.1', 'edit value true', 'activate',
    'read all', 'focus creche.birth', 'activate',
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
      'Birth Body requested']) {
      assert.ok(transcript.includes(required), `Birth transcript lacks ${required}`);
    }
  }
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
  const finalReading = attestReading(transcript, bornFace.presentation, ownerPart,
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
    speaker_playback_selected: Boolean(speakerCard),
    human_hearing_observed: false,
    owner_instance_speech_realization_observed: false,
    final_reading: finalReading,
    input: { path: '../birth-input.txt', sha256: digest(Buffer.from(input)) },
    transcript: { path: '../birth-transcript.txt', sha256: digest(Buffer.from(transcript)) },
  };
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
    '--speech-data', speechData, '--speech-engine', speechEngine);
  if (model) liveArgs.push('--model', model, '--ollama-endpoint', modelEndpoint,
    '--admitted-memory-mib', modelMemory);
  invoke(xtask, liveArgs, { timeout: 180_000 });
  const reportFile = path.join(live, 'report.json');
  const report = await load(reportFile);
  assert.equal(report.body_id, bodyId);
  assert.equal(report.owner_host_id, ownerPart.host_id);
  assert.equal(report.owner_boot_id, ownerPart.boot_id);
  assert.equal(report.native_source_commit, installation.release_source_identity);
  report.birth = birth;
  const beforeStart = ownerJson(['body', 'face', '--state-dir', state, '--json']);
  assert.equal(beforeStart.presentation.basis.body_id, bodyId);
  assert.equal(beforeStart.advertisement.host_id, ownerPart.host_id);
  assert.equal(beforeStart.advertisement.boot_id, ownerPart.boot_id);
  const available = (face, intent) => face.presentation.actions.find(action =>
    action.intent === intent && action.availability === 'Available');
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
    assert.equal(Number(enacted[0][2]), before.presentation.revision);
    assert.match(transcript, /Owner action result:/);
    const after = ownerJson(['body', 'face', '--state-dir', state, '--json']);
    assert.equal(after.presentation.basis.body_id, bodyId);
    assert.ok(after.presentation.revision > before.presentation.revision);
    const reading = attestReading(transcript, after.presentation, ownerPart,
      `screen-free clock ${name}`, true);
    assert.equal(reading.first.face_revision, before.presentation.revision);
    assert.equal(enacted[0][3], reading.first.source_show_id);
    return {
      action_id: action.identity, source_face_id: before.presentation.identity,
      source_face_revision: before.presentation.revision, source_show_id: enacted[0][3],
      result_face_id: after.presentation.identity,
      result_face_revision: after.presentation.revision,
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
  assert.ok(available(lull.after, 'conduit.intent/start-clock@1'));
  delete start.after;
  delete lull.after;
  report.screen_free_clock = {
    proof_class: speakerCard ? 'installed-screen-free-clock-selected-alsa' :
      'installed-screen-free-clock-text-readout',
    run_id: report.run_id, body_id: bodyId,
    owner_host_id: ownerPart.host_id, owner_boot_id: ownerPart.boot_id,
    source_commit: installation.release_source_identity,
    start, lull,
    speaker_playback_selected: Boolean(speakerCard), human_hearing_observed: false,
  };
  const walkthrough = await writeThreeHostWalkthrough(live, handbook, report);
  report.walkthrough = {
    ...walkthrough,
    sha256: digest(await readFile(path.join(live, walkthrough.path))),
    assets: await Promise.all(['conduit.css', 'chrome.css'].map(async file => ({
      path: file, sha256: digest(await readFile(path.join(live, file))),
    }))),
  };
  await writeFile(reportFile, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`Screen-free Birth and three-host proof: ${reportFile}`);
} finally {
  if (invitation?.exitCode === null) invitation.kill();
  if (service?.exitCode === null) service.kill();
}
