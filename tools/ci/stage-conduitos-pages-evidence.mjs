#!/usr/bin/env node

// Derive documentary pages only from an exact, completed QMP journey retained
// by the independently verified ConduitOS product. The release asset remains
// byte-for-byte the tested product; Pages receives this checked composition.
import { createHash } from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
import { cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { validateNativeTextEditReceipt } from './conduitos-text-edit-receipt.mjs';

const STEPS = Object.freeze([
  step('front-door-ready', 'Arrive at Crèche', 'Crèche is open before any Body exists.', 'The user sees a name field and locally available Plots.', 'A fresh Host and Boot have presented the first Face.', 'First boot'),
  step('body-born', 'Birth a Body', 'The selected Body is born and resting.', 'The user activates Birth after reviewing the Crèche Face.', 'Birth retains the Body without silently starting a Play.', 'Birth'),
  step('body-woken', 'Wake it', 'The same Body has accepted Wake.', 'The user presses F4.', 'Wake is a distinct lifecycle transition.', 'F4'),
  step('wake-refused', 'Refuse an unavailable Wake', 'The current refusal remains inspectable after the application surface returns.', 'The user presses F4 again while the Body is already Awake.', 'The owner refuses the unavailable action without changing the Body or its lifecycle state.', 'F4', false),
  step('body-planned', 'Plan its work', 'An exact Plan now names gears, ports, and cords.', 'The user presses F5.', 'The graph comes from the admitted Plan rather than a gallery specimen.', 'F5'),
  step('body-playing', 'Start Play', 'The Body awaits keyboard input in its admitted Play.', 'The user presses F6.', 'The Body identity persists across Wake, Plan, and Play.', 'F6'),
  step('protected-keyboard-canvas', 'Type into Keyboard canvas', 'Keyboard canvas presents an uppercase A.', 'The user selects Keyboard canvas and presses A.', 'The checked keyboard Plot executes within the same admitted Body Plan and Play.', 'Tab · A'),
  step('protected-memory-lantern', 'Edit retained text', 'Memory Lantern retains café with a native focus border and insertion caret.', 'The user selects Memory Lantern, edits text, then types café using the authored Right Alt mapping.', 'The editor retains bounded private state; the captured receipt records the committed Unicode bytes.', 'Tab · A · Backspace · C · A · F · Right Alt+E'),
  step('stale-refused', 'Refuse the old Face', 'The old Face refuses Enter after the current owner advances, without applying Stop.', 'The user holds Shift, leaves the editor with F1, opens the ordinary inspector and Details with F2 twice, focuses Stop with Shift+Tab, then releases Shift and presses Enter in one input report.', 'The stale interaction retains its original Face and Show; the newer owner Face admits zero effects, and the later fresh Stop proves recovery.', 'Held Shift · F1 · F2 · F2 · Shift+Tab · Shift release + Enter'),
  step('home', 'Go Home', 'Home opens while the Body continues to exist.', 'The user presses Escape.', 'Navigation changes the Face, not the Body identity.', 'Escape'),
  step('patchbay-workspace', 'Open Patchbay', 'The resident Patchbay becomes the foreground application.', 'The user selects Patchbay in Home and presses Enter.', 'The same Body and Plan remain current.', 'Tab · Enter'),
  step('patchbay-face', 'Inspect the Face', 'The native Mask presents the current semantic Face.', 'The user presses F2.', 'Content, actions, and structural relationships are projected from the running application.', 'F2'),
  step('patchbay-diagram', 'See the live diagram', 'Cards show planned gears and ports with visible cords.', 'The user presses F3 within the Face.', 'The diagram is rendered from this Face’s exact graph subjects and endpoints.', 'F3'),
  step('patchbay-gear-details', 'Inspect a Gear', 'The selected Gear opens with its exact Fore and placement details.', 'The user presses 1 in the live diagram.', 'Inspection retains the same Face, Body, Plan, and Play.', '1'),
  step('patchbay-diagram-returned', 'Return to the diagram', 'The same live planned graph is visible again.', 'The user presses F3.', 'Returning from details preserves the exact inspected Face and graph.', 'F3'),
  step('body-stopped', 'Stop the Play', 'The same Body is resting after Stop.', 'The user requests Stop through the Face with F8.', 'Stop leaves the Body available for a later Wake.', 'F8'),
  step('home-returned', 'Return Home', 'Home opens after the Play stops.', 'The user leaves Face inspection and presses Escape.', 'The original Body and born Sign remain retained.', 'Escape · Escape'),
  step('patchbay-reentered', 'Reenter Patchbay', 'The same resident Patchbay opens again.', 'The user selects Patchbay from Home.', 'Reentry preserves the original Body without another Birth.', 'Tab · Enter'),
]);

const [evidenceRoot, siteRoot, commit] = process.argv.slice(2);
if (!evidenceRoot || !siteRoot || !/^[0-9a-f]{40}$/.test(commit ?? '')) {
  throw new Error('usage: stage-conduitos-pages-evidence.mjs EVIDENCE_FRAMES SITE 40_HEX_COMMIT');
}
const manifest = JSON.parse(await readFile(path.join(evidenceRoot, 'manifest.json'), 'utf8'));
const proof = JSON.parse(await readFile(path.join(evidenceRoot, '..', 'journey-proof.json'), 'utf8'));
if (manifest.schema !== 'conduit.conduitos/visual-journey@1'
  || manifest.proof_class !== 'freestanding-emulator' || manifest.status !== 'complete'
  || manifest.failure !== null || manifest.context?.source_commit !== commit
  || proof.schema !== 'conduit.conduitos/face-journey-proof@1'
  || proof.source_commit !== commit || proof.proof_class !== 'freestanding-emulator'
  || proof.image_sha256 !== manifest.context.image_sha256
  || proof.screenshots !== 'journey-frames/manifest.json'
  || !Array.isArray(manifest.checkpoints)
  || manifest.checkpoints.length !== STEPS.length) {
  throw new Error('ConduitOS visual journey is incomplete or belongs to another product');
}
for (const [index, expected] of STEPS.entries()) {
  const entry = manifest.checkpoints[index];
  if (entry?.checkpoint !== expected.name
    || entry.health_refusal !== null || entry.unchanged !== false
    || entry.expected_change !== (expected.expectedChange ?? index !== 0)
    || entry.guest_boot_record?.boot_id !== proof.boot_id
    || entry.frame?.checkpoint !== expected.name
    || entry.frame.width !== 1280 || entry.frame.height !== 800
    || entry.frame.pixel_format !== 'RGBA8'
    || !(entry.frame.non_background_pixels > 64)
    || entry.frame.png !== `${expected.name}.png`
    || !/^[0-9a-f]{64}$/.test(entry.frame.png_sha256 ?? '')) {
    throw new Error(`ConduitOS checkpoint ${index + 1} violates the captured journey`);
  }
  const bytes = await readFile(path.join(evidenceRoot, entry.frame.png));
  if (bytes.length !== entry.frame.png_bytes || digest(bytes) !== entry.frame.png_sha256
    || !bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))
    || bytes.toString('ascii', 12, 16) !== 'IHDR'
    || bytes.readUInt32BE(16) !== 1280 || bytes.readUInt32BE(20) !== 800) {
    throw new Error(`ConduitOS screenshot ${expected.name} differs from its capture`);
  }
}
const serial = await readFile(path.join(evidenceRoot, '..', 'journey-serial.log'));
if (serial.length < 64 || serial.length > 16 * 1024 * 1024
  || typeof proof.boot_id !== 'string' || typeof proof.body_id !== 'string'
  || !serial.includes('CONDUIT_PRODUCT_JOURNEY')
  || !serial.includes(proof.boot_id) || !serial.includes(proof.body_id)) {
  throw new Error('ConduitOS journey serial transcript does not match the captured Body and Boot');
}

const pairedBytes = await readFile(path.join(evidenceRoot, '..', 'native-shell-paired-linear.json'));
if (pairedBytes.length > 32 * 1024 * 1024) throw new Error('ConduitOS paired evidence exceeds its retained bound');
const paired = JSON.parse(pairedBytes);
validatePaired(paired, proof, serial.toString('utf8'));
const editReceiptBytes = await readFile(path.join(evidenceRoot, '..', 'memory-lantern-native-edit.json'));
validateNativeTextEditReceipt(proof, JSON.parse(editReceiptBytes), serial);

const styles = await readFile('targets/browser/host/assets/conduit.css', 'utf8')
  + '\n' + await readFile('site/chrome.css', 'utf8');
const navigation = await readFile('site/navigation.html', 'utf8');
const current = path.join(siteRoot, 'journeys/current/conduitos/x86_64');
const retained = path.join(siteRoot, `journeys/commits/${commit}/conduitos/x86_64`);
for (const root of [current, retained]) {
  await mkdir(root, { recursive: true });
  // Preserve a prior console manifest when one exists. The transcript linked
  // from this page belongs to the same QMP session as these screens.
  try {
    const previous = JSON.parse(await readFile(path.join(root, 'manifest.json'), 'utf8'));
    if (previous.schema !== 'conduit.conduitos/visual-journey@1') {
      await cp(path.join(root, 'manifest.json'), path.join(root, 'console-manifest.json'));
    }
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  await cp(path.join(evidenceRoot, 'manifest.json'), path.join(root, 'manifest.json'), { force: true });
  await cp(path.join(evidenceRoot, '..', 'journey-proof.json'), path.join(root, 'journey-proof.json'), { force: true });
  await writeFile(path.join(root, 'console.txt'), serial);
  await writeFile(path.join(root, 'native-shell-paired-linear.json'), pairedBytes);
  await cp(path.join(evidenceRoot, '..', 'journey-qmp.log'), path.join(root, 'journey-qmp.log'), { force: true });
  await writeFile(path.join(root, 'memory-lantern-native-edit.json'), editReceiptBytes);
  for (const entry of STEPS) {
    await cp(path.join(evidenceRoot, `${entry.name}.png`), path.join(root, `${entry.name}.png`), { force: true });
  }
}
await writeJourney(current, '../../../', 'Current ConduitOS journey');
await writeJourney(retained, '../../../../', 'Retained ConduitOS journey');
const cardPath = path.join(siteRoot, 'journeys/index.html');
const marker = '<!-- conduit-conduitos-journey-card@1 -->';
const html = await readFile(cardPath, 'utf8');
if (html.split(marker).length !== 2) throw new Error('Journeys index needs one ConduitOS card slot');
const card = `<article class="journey-card"><p class="eyebrow">ConduitOS · QEMU · ${STEPS.length} captured steps</p><h2>Birth, Play, and inspect a Body</h2><p>Follow actual keyboard actions from Crèche into Patchbay and its live diagram.</p><p><a href="current/conduitos/x86_64/">Follow the ConduitOS journey</a></p></article>`;
await writeFile(cardPath, html.replace(marker, card));
const publicationPath = path.join(siteRoot, 'site-publication.json');
const publication = JSON.parse(await readFile(publicationPath, 'utf8'));
if (publication.sourceCommit !== commit) throw new Error('Website source differs from the ConduitOS journey');
publication.conduitos = {
  sourceCommit: commit, proofClass: 'freestanding-emulator',
  imageSha256: proof.image_sha256, serialSha256: digest(serial),
  committedEditSha256: digest(editReceiptBytes), committedEditPath: 'journeys/current/conduitos/x86_64/memory-lantern-native-edit.json',
  pairedSha256: digest(pairedBytes), pairedPath: 'journeys/current/conduitos/x86_64/native-shell-paired-linear.json',
  path: 'journeys/current/conduitos/x86_64/',
};
await writeFile(publicationPath, JSON.stringify(publication, null, 2) + '\n');

async function writeJourney(root, home, title) {
  const steps = STEPS.map((entry, index) => `<li class="journey-step" id="${entry.name}"><h2>${escape(entry.title)}</h2><p><strong>You do:</strong> ${escape(entry.action)}. ${escape(entry.happened)}</p><p>${escape(entry.seeing)}</p><figure><a href="${entry.name}.png"><img src="${entry.name}.png" alt="${escape(entry.seeing)}" loading="${index === 0 ? 'eager' : 'lazy'}"></a><figcaption>Captured from the same QMP guest after this action.</figcaption></figure><p><strong>What changed:</strong> ${escape(entry.proves)}</p></li>`).join('');
  const body = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title} · Conduit</title><style>${styles}</style></head><body data-application-look="conduit.look/phosphor@1">${navigation}<main class="site-content"><p><a href="${home}">All journeys</a> · <a href="/conduit/workspace/">Open Conduit</a> · <a href="/conduit/handbook/">Handbook</a></p><p class="eyebrow">Freestanding x86 QEMU · ${STEPS.length} screenshots</p><h1>A Body, its Patchbay, and its Face</h1><p class="lede">Arrive at Crèche, birth a Body, start its work, inspect the live planned graph, and stop it. Each screen was captured from one keyboard-driven guest session.</p><p class="recording-note">Emulator evidence from source <code>${commit}</code>. It does not establish physical hardware operation or human enactment. Semantic pointer input and a source-equivalent installed browser route remain unsupported; parent #5356 acceptance remains open.</p><ol class="journey-steps">${steps}</ol><h2>Reproduce and inspect</h2><p><code>cargo +stable xtask make conduitos graphical-profile-proof</code></p><p><a href="manifest.json">Screenshot provenance</a> · <a href="journey-proof.json">Journey receipt</a> · <a href="console.txt">Journey serial transcript</a> · <a href="journey-qmp.log">Real QMP keyboard input</a> · <a href="native-shell-paired-linear.json">Source Face, native action receipts and paired linear reconstruction</a> · <a href="memory-lantern-native-edit.json">Committed text receipt</a></p></main></body></html>`;
  await writeFile(path.join(root, 'index.html'), body);
}

function step(name, title, seeing, happened, proves, action, expectedChange) {
  return Object.freeze({ name, title, seeing, happened, proves, action, expectedChange });
}
function digest(bytes) { return createHash('sha256').update(bytes).digest('hex'); }
function escape(value) {
  return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;').replaceAll('"', '&quot;');
}

function validatePaired(paired, proof, serial) {
  const fail = () => { throw new Error('ConduitOS paired evidence differs from the enacted source, Body or action outcomes'); };
  if (proof.paired_reconstruction !== 'native-shell-paired-linear.json'
    || paired.schema !== 'conduit.native-shell/paired-linear@1'
    || paired.proof_class !== 'freestanding-emulator' || paired.source_commit !== proof.source_commit
    || paired.image_sha256 !== proof.image_sha256 || paired.body_id !== proof.body_id
    || paired.screenshots !== 'journey-frames/manifest.json'
    || paired.birth_count !== 1 || !Array.isArray(paired.actions) || !Array.isArray(paired.paired_faces)
    || paired.pointer?.status !== 'unsupported' || paired.browser?.status !== 'unsupported'
    || paired.parent_5356_acceptance !== 'open' || paired.physical_evidence !== false || paired.human_enactment !== false) fail();
  const records = prefix => serial.split('\n').filter(line => line.startsWith(prefix))
    .map(line => JSON.parse(line.slice(prefix.length)));
  const faces = records('CONDUIT_NATIVE_FACE ');
  const actions = records('CONDUIT_NATIVE_ACTION ');
  const births = records('CONDUIT_NATIVE_BIRTH ');
  if (births.length !== 1 || !isDeepStrictEqual(paired.birth, births[0])
    || !isDeepStrictEqual(paired.actions, actions) || paired.paired_faces.length !== faces.length || !faces.length) fail();
  for (const [index, pair] of paired.paired_faces.entries()) {
    if (!isDeepStrictEqual(pair.native, faces[index])
      || pair.native.show?.lifecycle !== 'Available'
      || pair.linear?.proof_class !== 'deterministic-semantic-reconstruction'
      || pair.linear.face_id !== pair.native.face?.identity || pair.linear.face_revision !== pair.native.face?.revision
      || !isDeepStrictEqual(pair.linear.actions, pair.native.face.actions)
      || !isDeepStrictEqual(pair.linear.basis, pair.native.face.basis)
      || !isDeepStrictEqual(pair.linear.context, pair.native.face.interaction_context)
      || !Array.isArray(pair.linear.lines) || pair.linear.lines.length === 0
      || pair.linear.acknowledged_show !== null || pair.linear.input_enactment !== false) fail();
  }
  const birth = paired.birth;
  if (birth.schema !== 'conduit.native-action/v1' || birth.accepted_effects !== 1
    || !birth.disposition?.Accepted || birth.before?.body_id !== null
    || birth.before.born_sign_id !== null || birth.after?.body_id !== proof.body_id
    || typeof birth.after.born_sign_id !== 'string') fail();
  let accepted = 0; let refused = 0; let recovered = false;
  let unavailable = 0; let staleRecovered = false; const stale = [];
  for (const action of [birth, ...actions]) {
    if (action.schema !== 'conduit.native-action/v1'
      || !faces.some(record => record.show.show_id === action.show_id
        && record.face.identity === action.face_id && record.face.revision === action.face_revision)) fail();
    if (action === birth) continue;
    for (const side of ['before', 'after']) {
      if (action[side]?.body_id !== proof.body_id || action[side].born_sign_id !== birth.after.born_sign_id) fail();
    }
    if (action.disposition?.Accepted) {
      if (action.accepted_effects !== 1 || isDeepStrictEqual(action.before, action.after)) fail();
      accepted++; recovered ||= refused > 0; staleRecovered ||= stale.length > 0;
    } else if (action.disposition?.Refused) {
      if (action.accepted_effects !== 0 || !isDeepStrictEqual(action.before, action.after)
        || typeof action.disposition.Refused.reason !== 'string' || !action.disposition.Refused.reason) fail();
      const reason = action.disposition.Refused.reason;
      if (reason === 'UnavailableAction') unavailable++;
      else if (reason === 'StaleFace') {
        const source = faces.find(record => record.show.show_id === action.show_id);
        const interaction = action.interaction, admission = action.admission_face;
        if (!interaction || source.mask_show?.show_id !== action.show_id
          || interaction.show_id !== action.show_id || interaction.face_id !== source.face.identity
          || interaction.face_revision !== source.face.revision
          || !source.face.actions.some(candidate => candidate.identity === interaction.action_id
            && candidate.target === interaction.target && candidate.availability === 'Available')
          || !admission || typeof admission.identity !== 'string' || admission.identity === source.face.identity
          || !(admission.revision > source.face.revision) || admission.basis?.body_id !== action.before.body_id
          || !Array.isArray(admission.actions) || !admission.interaction_context) fail();
        stale.push(action); staleRecovered = false;
      } else fail();
      refused++;
    } else fail();
  }
  const replay = paired.stale_show_replay;
  if (!unavailable || !stale.length || !staleRecovered || replay?.installed_enactment !== true
    || replay.validated_event_count !== stale.length || replay.fresh_action_after_refusal !== staleRecovered
    || !Array.isArray(replay.admission_faces) || replay.admission_faces.length !== stale.length) fail();
  for (const [index, action] of stale.entries()) {
    const retained = replay.admission_faces[index], admission = action.admission_face, linear = retained?.linear;
    if (retained?.interaction_id !== action.interaction.identity
      || !isDeepStrictEqual(retained.admission_face, admission)
      || linear?.proof_class !== 'deterministic-semantic-reconstruction'
      || linear.face_id !== admission.identity || linear.face_revision !== admission.revision
      || !isDeepStrictEqual(linear.actions, admission.actions)
      || !isDeepStrictEqual(linear.basis, admission.basis)
      || !isDeepStrictEqual(linear.context, admission.interaction_context)
      || !Array.isArray(linear.lines) || !linear.lines.length
      || linear.acknowledged_show !== null || linear.input_enactment !== false) fail();
  }
  if (!accepted || !refused || !recovered || paired.accepted_action_count !== accepted
    || paired.refused_action_count !== refused || paired.fresh_action_after_refusal !== recovered) fail();
}
