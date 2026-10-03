#!/usr/bin/env node

// Derive documentary pages only from an exact, completed QMP journey retained
// by the independently verified ConduitOS product. The release asset remains
// byte-for-byte the tested product; Pages receives this checked composition.
import { createHash } from 'node:crypto';
import { cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const STEPS = Object.freeze([
  step('front-door-ready', 'Arrive at Crèche', 'Crèche is open before any Body exists.', 'The user sees a name field and locally available Plots.', 'A fresh Host and Boot have presented the first Face.', 'First boot'),
  step('body-born', 'Birth a Body', 'The selected Body is born and resting.', 'The user activates Birth after reviewing the Crèche Face.', 'Birth retains the Body without silently starting a Play.', 'Birth'),
  step('body-woken', 'Wake it', 'The same Body has accepted Wake.', 'The user presses F4.', 'Wake is a distinct lifecycle transition.', 'F4'),
  step('body-planned', 'Plan its work', 'An exact Plan now names gears, ports, and cords.', 'The user presses F5.', 'The graph comes from the admitted Plan rather than a gallery specimen.', 'F5'),
  step('body-playing', 'Start Play', 'The Body awaits keyboard input in its admitted Play.', 'The user presses F6.', 'The Body identity persists across Wake, Plan, and Play.', 'F6'),
  step('home', 'Go Home', 'Home opens while the Body continues to exist.', 'The user presses Escape.', 'Navigation changes the Face, not the Body identity.', 'Escape'),
  step('patchbay-workspace', 'Open Patchbay', 'The resident Patchbay becomes the foreground application.', 'The user selects Patchbay in Home and presses Enter.', 'The same Body and Plan remain current.', 'Tab · Enter'),
  step('patchbay-face', 'Inspect the Face', 'The native Mask presents the current semantic Face.', 'The user presses F2.', 'Content, actions, and structural relationships are projected from the running application.', 'F2'),
  step('patchbay-diagram', 'See the live diagram', 'Cards show planned gears and ports with visible cords.', 'The user presses F3 within the Face.', 'The diagram is rendered from this Face’s exact graph subjects and endpoints.', 'F3'),
  step('body-stopped', 'Stop the Play', 'The same Body is resting after Stop.', 'The user requests Stop through the Face with F8.', 'Stop leaves the Body available for a later Wake.', 'F8'),
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
    || entry.expected_change !== (index !== 0)
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

const styles = await readFile('targets/browser/host/assets/conduit.css', 'utf8')
  + '\n' + await readFile('site/chrome.css', 'utf8');
const navigation = await readFile('site/navigation.html', 'utf8');
const current = path.join(siteRoot, 'journeys/current/conduitos/x86_64');
const retained = path.join(siteRoot, `journeys/commits/${commit}/conduitos/x86_64`);
for (const root of [current, retained]) {
  await mkdir(root, { recursive: true });
  // The earlier console proof remains available even though this journey now
  // leads with real captured screens.
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
const card = '<article class="journey-card"><p class="eyebrow">ConduitOS · QEMU · ten captured steps</p><h2>Birth, Play, and inspect a Body</h2><p>Follow actual keyboard actions from Crèche into Patchbay and its live diagram.</p><p><a href="current/conduitos/x86_64/">Follow the ConduitOS journey</a></p></article>';
await writeFile(cardPath, html.replace(marker, card));
const publicationPath = path.join(siteRoot, 'site-publication.json');
const publication = JSON.parse(await readFile(publicationPath, 'utf8'));
if (publication.sourceCommit !== commit) throw new Error('Website source differs from the ConduitOS journey');
publication.conduitos = {
  sourceCommit: commit, proofClass: 'freestanding-emulator',
  imageSha256: proof.image_sha256, path: 'journeys/current/conduitos/x86_64/',
};
await writeFile(publicationPath, JSON.stringify(publication, null, 2) + '\n');

async function writeJourney(root, home, title) {
  const steps = STEPS.map((entry, index) => `<li class="journey-step" id="${entry.name}"><h2>${escape(entry.title)}</h2><p><strong>You do:</strong> ${escape(entry.action)}. ${escape(entry.happened)}</p><p>${escape(entry.seeing)}</p><figure><a href="${entry.name}.png"><img src="${entry.name}.png" alt="${escape(entry.seeing)}" loading="${index === 0 ? 'eager' : 'lazy'}"></a><figcaption>Captured from the same QMP guest after this action.</figcaption></figure><p><strong>What changed:</strong> ${escape(entry.proves)}</p></li>`).join('');
  const body = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title} · Conduit</title><style>${styles}</style></head><body data-application-theme="conduit.presentation/phosphor@1">${navigation}<main class="site-content"><p><a href="${home}">All journeys</a> · <a href="/conduit/workspace/">Open Conduit</a> · <a href="/conduit/handbook/">Handbook</a></p><p class="eyebrow">Freestanding x86 QEMU · ten screenshots</p><h1>A Body, its Patchbay, and its Face</h1><p class="lede">Arrive at Crèche, birth a Body, start its work, inspect the live planned graph, and stop it. Each screen was captured from one keyboard-driven guest session.</p><p class="recording-note">Emulator evidence from source <code>${commit}</code>. It does not establish physical hardware operation or human enactment.</p><ol class="journey-steps">${steps}</ol><h2>Reproduce and inspect</h2><p><code>cargo xtask make conduitos journey-proof</code></p><p><a href="manifest.json">Screenshot provenance</a> · <a href="journey-proof.json">Journey receipt</a> · <a href="console.txt">Retained console</a></p></main></body></html>`;
  await writeFile(path.join(root, 'index.html'), body);
}

function step(name, title, seeing, happened, proves, action) {
  return Object.freeze({ name, title, seeing, happened, proves, action });
}
function digest(bytes) { return createHash('sha256').update(bytes).digest('hex'); }
function escape(value) {
  return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;').replaceAll('"', '&quot;');
}
