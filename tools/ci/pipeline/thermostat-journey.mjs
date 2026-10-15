import { createHash } from 'node:crypto';
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { parseCaptureJSON } from '../../../proof/browser/exact-capture-json.mjs';
import { validateTerminalCapture } from './thermostat-terminal-capture.mjs';
import { validateSpokenCapture } from './thermostat-spoken-capture.mjs';
import { requireExactAncestor } from './exact-ancestor.mjs';

export const THERMOSTAT_EVIDENCE_ROOT = 'site/evidence/thermostat-journey';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const escape = text => String(text).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;');
const insist = (ok, message) => { if (!ok) throw new Error(`Thermostat journey: ${message}`); };

// This validator checks retained observations, not fresh execution or release acceptance.
export function validateThermostatJourney(root) {
  insist(!lstatSync(root).isSymbolicLink() && !lstatSync(path.join(root, 'capture')).isSymbolicLink(), 'symlinked capture directory');
  insist(lstatSync(path.join(root, 'manifest.json')).isFile() && lstatSync(path.join(root, 'manifest.json')).size <= 64 * 1024, 'manifest exceeds bound');
  const manifest = parseCaptureJSON(readFileSync(path.join(root, 'manifest.json'), 'utf8'));
  insist(manifest.schema === 'conduit.thermostat-journey/capture@2'
    && manifest.result === 'browser-terminal-spoken-complete'
    && /^[a-f0-9]{40}$/.test(manifest.source_commit)
    && Array.isArray(manifest.files) && manifest.files.length > 0 && manifest.files.length <= 64
    && manifest.capture_tool_dirty === false && manifest.capture_tool_commit === manifest.source_commit
    && manifest.source_relation === 'exact-source', 'missing capture identity or inventory');
  const paths = new Set();
  let total = 0;
  for (const file of manifest.files) {
    insist(/^capture\/[a-z0-9][a-z0-9.-]*$/.test(file.path) && !paths.has(file.path), 'unsafe or duplicate capture path');
    paths.add(file.path);
    const filename = path.join(root, file.path);
    insist(lstatSync(filename).isFile() && !lstatSync(filename).isSymbolicLink(), 'capture must be an ordinary file');
    insist(lstatSync(filename).size <= 16 * 1024 * 1024, 'capture digest or bounds differ');
    const bytes = readFileSync(filename);
    total += bytes.length;
    insist(bytes.length <= 16 * 1024 * 1024 && file.bytes === bytes.length && file.sha256 === hash(bytes), 'capture digest or bounds differ');
  }
  insist(total <= 128 * 1024 * 1024, 'capture exceeds total bound');
  insist(!lstatSync(path.join(root, 'capture')).isSymbolicLink(), 'symlinked capture directory');
  insist(readdirSync(path.join(root, 'capture')).every(name => paths.has(`capture/${name}`)), 'unlisted capture output');
  const required = ['receipt.json', 'terminal.txt', 'thermostat-desktop.png', 'thermostat-raised.png',
    'thermostat-mobile.png', 'thermostat-body-menu.png', 'thermostat-after-terminal.png', 'thermostat-lulled.png', 'thermostat-restarted.png'];
  for (const name of required) insist(paths.has(`capture/${name}`), `missing ${name}`);
  for (const name of required.filter(name => name.endsWith('.png'))) {
    insist(readFileSync(path.join(root, 'capture', name)).subarray(0, 8).equals(Buffer.from('89504e470d0a1a0a', 'hex')), 'invalid PNG capture');
  }
  const receipt = parseCaptureJSON(readFileSync(path.join(root, 'capture/receipt.json'), 'utf8'));
  insist(receipt.schema === 'conduit.proof/thermostat-owner-browser@2'
    && receipt.owner_source_commit === manifest.source_commit
    && receipt.browser_runtime_source_commit === manifest.source_commit
    && receipt.handbook_ui_source_commit === manifest.source_commit && receipt.handbook_ui_source_clean === true
    && receipt.source_relation === 'exact-source'
    && new Set([receipt.owner_host_id, receipt.presentation_peer_host_id, receipt.browser_host_id]).size === 3
    && receipt.body_id === manifest.body_id && receipt.body_plan_id === manifest.body_plan_id
    && receipt.body_play_id === manifest.body_play_id && receipt.physical_sensor_or_equipment === false, 'receipt identity differs');
  const journey = receipt.journey;
  insist(journey && receipt.revisions.length > 0, 'missing cross-Mask observations');
  for (const observation of receipt.revisions) {
    insist(observation.view.body_id === manifest.body_id && observation.owner.face.basis.body_id === manifest.body_id
      && observation.view.face_id === observation.owner.face.identity
      && observation.retained_body_execution.body_id === manifest.body_id
      && observation.retained_body_execution.plan_id === manifest.body_plan_id
      && observation.retained_body_execution.active_play_id === manifest.body_play_id, 'cross-Mask observations changed Body or Play');
  }
  const terminal = validateTerminalCapture(root, receipt);
  insist(journey.terminal_evidence[0].face_id === journey.terminal_before.face.identity
    && journey.terminal_evidence[1].face_id === journey.terminal_stale.after.face.identity
    && journey.terminal_evidence[2].face_id === journey.after_terminal.view.face_id, 'terminal Shows differ from observed Faces');
  insist(journey.terminal_stale.accepted === false && receipt.stale_show.accepted === false, 'missing stale-Show refusal');
  insist(terminal.includes('18 °C'), 'terminal did not observe accepted Eco target');
  const target = view => view.subjects.find(subject => subject.name === 'Target temperature')?.values?.find(value => value.kind === 'value/quantity@1')?.text;
  insist(target(journey.before_terminal.view) === '22.5 °C' && target(journey.after_terminal.view) === '18 °C' && target(journey.restarted.view) === '21 °C', 'missing cross-Mask target or reset');
  insist(journey.browser_stale.accepted === false
    && journey.terminal_stale.code === 'stale-face-or-show'
    && JSON.stringify(journey.browser_stale.before.face.properties) === JSON.stringify(journey.browser_stale.after.face.properties)
    && JSON.stringify(journey.terminal_stale.before.face.properties) === JSON.stringify(journey.terminal_stale.after.face.properties)
    && JSON.stringify(journey.terminal_actions.map(action => action.accepted)) === '[false,true,true]', 'missing stale-Show refusal or accepted edits');
  insist(journey.after_peer_closed.biography.body_id === manifest.body_id
    && journey.after_peer_closed.realization.plan.plan_id === manifest.body_plan_id
    && journey.after_peer_closed.realization.play.active_play_id === manifest.body_play_id, 'peer close changed Body or Play');
  validateSpokenCapture(root, receipt, paths);
  insist(journey.lulled.status.realization === null
    && !journey.lulled.owner.face.actions.some(action => action.identity.startsWith('thermostat.'))
    && journey.restarted.status.biography.body_id === manifest.body_id
    && journey.restarted.status.realization.play.active_play_id
    && journey.restarted.status.realization.play.active_play_id !== manifest.body_play_id, 'missing lull and new-Play lifecycle');
  return { manifest, receipt };
}

export function renderThermostatJourney(root, destination, publicationSourceCommit, styles, navigation) {
  const { manifest, receipt } = validateThermostatJourney(root);
  insist(/^[a-f0-9]{40}$/.test(publicationSourceCommit), 'missing publication source');
  requireExactAncestor(manifest.source_commit, publicationSourceCommit, 'Thermostat capture');
  insist(!existsSync(destination), 'destination already exists');
  mkdirSync(destination, { recursive: true });
  cpSync(path.join(root, 'capture'), path.join(destination, 'capture'), { recursive: true });
  cpSync(path.join(root, 'manifest.json'), path.join(destination, 'manifest.json'));
  const steps = [
    ['start', 'Meet your thermostat', 'Start Living room and open its browser Mask.', 'The target starts at 21 °C. The sensor is unavailable; requested settings do not confirm equipment activity.', 'thermostat-desktop.png', 'Initial browser thermostat with requested settings'],
    ['adjust', 'Choose a warmer target', 'Raise the target three times, including one keyboard activation.', 'The browser reports 22.5 °C. Manual adjustment selects Custom; the same Body and Play continue.', 'thermostat-raised.png', 'Browser target raised to 22.5 °C'],
    ['terminal', 'Meet the same thermostat in a terminal', 'Wear and prefer the terminal Mask, then present its current Show.', 'The terminal reads the same retained thermostat. An earlier terminal Show is refused after the browser edit, and the terminal receives the current 22.5 °C Face.', null, null],
    ['eco', 'Change the request from the terminal', 'Select Heat, then Eco through the offered typed actions.', 'The requested target becomes 18 °C. The terminal action changes the existing scan rather than starting another thermostat.', null, null],
    ['speech', 'Hear the current settings', 'Read settings through the installed spoken Mask.', 'The checked reading describes the current 18 °C request. The retained recording joins the actual output frames in order; physical speaker delivery and human listening were not observed.', null, null],
    ['inspect', 'See the change in the browser', 'Prefer the browser Mask again and refresh its Face.', 'The browser shows 18 °C. Its old Show is refused and cannot overwrite the terminal change.', 'thermostat-mobile.png', 'thermostat-body-menu.png', 'thermostat-after-terminal.png', 'Browser observes the terminal’s Heat and Eco request'],
    ['lull', 'Put the thermostat to rest', 'Inspect the separate Body menu. Lull the running Play through the owner and refresh.', 'The thermostat controls are withdrawn. The Body remains available.', 'thermostat-lulled.png', 'Body after the thermostat Play is lulled'],
    ['return', 'Return to the same Body', 'Start a new Play and refresh the browser.', 'The Body keeps its identity. Session settings reset to the authored 21 °C initial state; the new Play has its own identity.', 'thermostat-restarted.png', 'Same Body with a fresh thermostat Play at 21 °C'],
  ];
  const cards = steps.map(([id, title, action, result, image, alt], index) => `<section id="${id}" class="journey-step"><p class="eyebrow">Step ${index + 1} of ${steps.length}</p><h2>${title}</h2><p><strong>You do:</strong> ${action}</p><p><strong>What changes:</strong> ${result}</p>${id === 'lull' ? '<figure><img src="capture/thermostat-body-menu.png" alt="Lull inside the separate Body menu" loading="lazy"></figure>' : ''}${image ? `<figure><img src="capture/${image}" alt="${escape(alt)}" loading="lazy"><figcaption>${escape(alt)} · <a href="manifest.json">Capture inventory</a></figcaption></figure>` : ''}${id === 'terminal' || id === 'eco' ? '<details><summary>Inspect the actual terminal capture</summary><pre>' + escape(readFileSync(path.join(root, 'capture/terminal.txt'), 'utf8')) + '</pre><a href="capture/terminal.txt">Complete terminal capture</a></details>' : ''}${id === 'speech' ? '<audio controls preload="none" src="capture/spoken-reading.wav">Recorded spoken settings</audio><p><a href="capture/spoken-reading.wav">Recorded output</a></p>' : ''}</section>`).join('');
  const setup = `https://github.com/dancxjo/conduit/blob/${publicationSourceCommit}/docs/journeys/thermostat.md`;
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Control one thermostat across Masks — Conduit</title><style>${styles}\n.thermostat-journey{max-width:72rem;margin:auto;padding:2rem 1.25rem 5rem}.thermostat-journey img{display:block;max-width:100%;height:auto}.thermostat-journey pre{white-space:pre-wrap;overflow-wrap:anywhere;max-height:30rem;overflow:auto}.thermostat-journey .journey-step{padding:2rem 0;border-top:1px solid var(--conduit-structure-secondary)}.thermostat-journey .boundary{padding:1rem;border-left:3px solid var(--conduit-emphasis)}.thermostat-journey h1{font-size:clamp(2rem,5vw,3.5rem);line-height:1.1}</style></head><body data-application-look="conduit.look/phosphor@1">${navigation}<main class="thermostat-journey" id="main-content"><p><a href="../../">All journeys</a> · <a href="../todo/">Todo across Masks</a></p><p class="eyebrow">One thermostat Body · browser, terminal and spoken capture</p><h1>Control one thermostat across Masks</h1><p class="lede">Adjust a temperature request in the browser, change it from a terminal, then see the same thermostat respond. Lull it and return to the same Body.</p><p class="boundary">This is local development evidence. Spoken output uses explicitly installed artifact audio equipment. Settings are session-local. No physical sensor, heating or cooling is confirmed.</p><p><a href="${setup}">Repeat this journey</a></p><nav aria-label="Journey steps"><ol>${steps.map(([id, title]) => `<li><a href="#${id}">${title}</a></li>`).join('')}</ol></nav>${cards}<details><summary>Source, receipts and limits</summary><p>The Owner source is <code>${manifest.source_commit}</code>; browser runtime source <code>${escape(receipt.browser_runtime_source_commit)}</code>; Handbook UI source <code>${escape(receipt.handbook_ui_source_commit)}</code>. Source relation: ${escape(receipt.source_relation)}. Capture-tool source <code>${escape(manifest.capture_tool_commit)}</code>${manifest.capture_tool_dirty ? ' with local changes' : ''}. This page verifies retained relationships and digests; it does not claim new execution or protected release acceptance.</p><p>Body <code>${escape(manifest.body_id)}</code> · original Play <code>${escape(manifest.body_play_id)}</code></p><p><a href="capture/receipt.json">Complete execution receipt</a> · <a href="manifest.json">Digest-bound inventory</a></p><p>The browser and two separately installed Linux Hosts ran on one physical machine. Returning after Lull is a new Play, not checkpoint recovery. The browser-local Handbook thermostat is a separate Body and is not used as a substitute in this journey.</p></details></main></body></html>`;  writeFileSync(path.join(destination, 'index.html'), html);
  return { sourceCommit: manifest.source_commit, bodyId: manifest.body_id, speechAvailable: true };
}
