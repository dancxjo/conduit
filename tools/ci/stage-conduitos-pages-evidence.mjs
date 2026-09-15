#!/usr/bin/env node

import { createHash } from "node:crypto";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const JOURNEY = Object.freeze([
  stage("front-door-ready", "Crèche ready", "The native Crèche offers a Body name, a naming tradition, and the locally reviewed keyboard Form.", "The image opened the Crèche as its first ordinary product surface.", "The checked Form was inspected while Body, Wake, Plan, and Play remained absent.", "Crèche · Host · Form", "Boot"),
  stage("body-awake", "Body awake", "The chosen Body is named and its included keyboard Form is listening.", "Return accepted the Crèche selection, birthed the Body, then requested Wake, Plan, and Play in order.", "Separate lifecycle records retain the exact Body, workset, Host, Boot, admitted Plan, and Play identities for each successful stage.", "Body · Wake · Plan · Play", "Return"),
  stage("host-current-offers", "Host offers inspected", "The exact-detail inspector lists the operations this running Host currently offers.", "F2 advanced through identity and binding details to CURRENT OFFERS.", "The visible inventory is projected from current Host truth while the same Body, Plan, and Play remain in place; it is not an authored Form claim or a catalog-wide availability promise.", "Host · offers · inspection · Plan", "F2"),
  stage("quiescent-awaiting-input", "Play quiescent", "The same admitted Play is quiescent and awaiting later input after its initial work drains.", "F2 opened details; Escape returned input focus to the Form without completing or replacing the Play.", "Inspection preserves the exact Body, Plan, and Play while structural drain remains distinct from semantic completion.", "Plan · Play · quiescence · focus", "F2 / Escape"),
  stage("input-continued", "Input continued", "The presentation shows HELLO while the same quiescent Play remains available for more input.", "Five keys crossed the admitted keyboard path, each with a press and release, and continued the existing Play.", "Every event retains the same Body, Plan, and Play; output is correlated with the port-specific kernel result without claiming completion.", "Form · Play · quiescence · presentation", "H E L L O"),
  stage("patchbay-current-canvas", "Patchbay shows the current canvas", "The resident Patchbay presents the active keyboard canvas and its live topology.", "F10 opened Patchbay from the running Body without replacing its current Form result.", "Patchbay projects the same Body, Plan, Play, and resident Form truth rather than fabricating a second runtime.", "Patchbay · Body · resident Forms", "F10"),
  stage("patchbay-edit-requested", "Patchbay edit requested", "Patchbay visibly acknowledges an edit request for the current Body.", "F11 crossed the ordinary application-action path while Patchbay was selected.", "The request is correlated with the current Body and remains distinct from topology realization or a new Play.", "Patchbay · edit request · Body", "F11"),
  stage("patchbay-presenters-replanned", "Presenter topology replanned", "Patchbay presents the Body with its newly realized Presenter topology.", "F1 requested an ordinary Presenter replan that prepared and started a fresh immutable Plan and Play before replacing the old route.", "The same Body keeps surviving manifestation continuity while exact Presenter, implementation, Host, Boot, resource, and availability truth comes from the new Plan.", "Patchbay · Presenters · Plan · Play", "F1"),
  stage("resident-tour-result", "Resident Tour result", "The resident Tour Form presents its HELLO result inside the same Body.", "The journey selected Tour and ran its admitted action through the shared application path.", "A distinct resident Form can run and retain output without replacing the Body or erasing the canvas result.", "Resident Forms · Tour · retained result", "Form switch + F10"),
  stage("memory-listening", "Memory Form listening", "The resident memory Form is selected and preserves the text ONE.", "The journey switched from the initial keyboard canvas Form to the memory lantern Form and typed one bounded value.", "Form selection, input focus, and retained output are separate from Body identity and Play admission.", "Resident Forms · focus · output", "O N E"),
  stage("canvas-retained", "Canvas result retained", "The keyboard canvas Form still presents HELLO after switching away and back.", "The journey returned to the canvas Form after editing the memory Form.", "Resident Forms retain independent results without turning view selection into a new Play or Body.", "Resident Forms · retained state", "Form switch"),
  stage("memory-cleared", "Memory Form cleared", "The memory Form remains selected after its value is erased.", "Backspace events removed the value previously entered in the memory Form.", "Edits are scoped to the active resident Form and do not mutate the canvas Form's retained value.", "Resident Forms · scoped input", "Backspace"),
  stage("patchbay-current-canvas-returned", "Patchbay shows retained canvas state", "Patchbay again presents the keyboard canvas after resident Forms were edited.", "The journey returned through Patchbay after changing memory and canvas values.", "Patchbay projects the current shared Body truth while each resident Form keeps its independently retained result.", "Patchbay · resident Forms · continuity", "Form switch"),
  stage("memory-retained", "Resident Forms retained", "The memory Form shows HI while the canvas Form separately retains HELLOXY.", "The journey edited both resident Forms, then returned to the memory Form.", "Each resident Form retains its own typed Port result across selection changes inside the same Body and Play.", "Resident Forms · Port result · retention", "Form switch"),
  stage("lulled", "Body Lulled", "The Body is retained and no Play is active.", "F8 explicitly stopped the Play; F7 then lulled the Body.", "Execution stopped without erasing Body or journey identity.", "Body · Lull · Play", "F8 / F7"),
  stage("usb-line-current", "USB Line current", "The product reports one current bounded Line backed by the emulated FTDI USB function.", "F12 opened the Line view after QEMU had supplied the separate USB function.", "Guest records establish the exact controller, endpoints, binding, Host, and Boot; the screenshot only shows their presentation.", "Cord · Line · ports", "F12"),
  stage("peer-attached", "Peer attached", "The Line view reports that the harness peer completed its bounded greeting.", "The peer exchanged the canonical Hello and Ready bytes over the emulated USB device.", "Bytes crossed the real emulated carrier, while the record keeps connectivity distinct from Body membership or trust.", "Line · peer · authority", "Hello / Ready"),
  stage("line-value-visible", "Line value visible", "HELLO USB LINE is manifested in the guest.", "The peer delivered one bounded value and acknowledged the session.", "Delivery is correlated to distinct Line, binding, Plan, active Play, Host, Boot, and unchanged Body identities.", "Line · value · Play", "Value + acknowledgment"),
  stage("line-lost", "Line lost", "The product visibly reports that the USB Line has disappeared.", "The harness used QMP to remove the separately pinned FTDI USB function.", "Removal is detected, the stale session is refused, and Body identity remains unchanged rather than turning loss into success.", "Line loss · stale identity", "QMP device removal"),
  stage("tour-opened", "Tour opened", "The canonical meet-one-gear lesson is open and has not run yet.", "F9 opened Tour from the ordinary product after the Body returned to Lulled.", "The same emulated keyboard and portable action path reaches the shared Tour application without creating a second runtime.", "Tour · Form · Gear", "F9"),
  stage("tour-result-visible", "Tour result visible", "Tour displays HELLO from its canonical Form.", "F10 ran the lesson.", "The production kernel result remains correlated to exact source, checked Form, expanded Form, Plan, and active Play identities.", "Tour · Form · Plan · Play", "F10"),
  stage("confirmation-transient", "Confirmation shown", "A separate confirmation surface is visible above the completed Tour result.", "The successful run manifested its bounded confirmation transient.", "The transient has its own surface identity and relationship without replacing the result or becoming application truth.", "Tour · transient · presentation", "Run completion"),
  stage("confirmation-dismissed", "Confirmation dismissed", "The confirmation is gone and the retained Tour result is visible again.", "Escape dismissed the exact confirmation surface.", "Dismissal closes only the transient while preserving the underlying result and workspace identities.", "Tour · transient closure · result", "Escape"),
  stage("tour-one-exercise-two", "Tour 1.2 completed", "The second lesson presents MAKE THIS LOUD through the same native Tour workspace.", "F3 selected the next exercise; F10 ran its authored Form; Escape dismissed its nonblank confirmation.", "The shared Tour program retains the exercise-specific source, checked Form, expanded Form, Plan, Play, and result identities.", "Tour · authored Form · retained result", "F3 / F10 / Escape"),
  stage("tour-one-exercise-three", "Tour 1.3 completed", "The third lesson presents SOS after executing its two-Gear Form.", "F3 selected the next exercise; F10 ran it; Escape dismissed the bounded confirmation.", "Multiple manifestations remain one exact planned application result rather than separate improvised programs.", "Tour · Gears · manifestations", "F3 / F10 / Escape"),
  stage("tour-two-exercise-one", "Tour 2.1 compared", "The comparison lesson reports that direct and recursive realizations agree.", "F5 advanced to chapter two; F10 executed both admitted realizations; Escape dismissed confirmation.", "The distinct expanded Form and Plan identities are retained while their observable results agree.", "Tour · comparison · Plan identities", "F5 / F10 / Escape"),
  stage("tour-three-exercise-one", "Tour 3.1 stopped", "The finite counter lesson presents 1 and records an explicit stopped terminal outcome.", "F5 advanced to chapter three; F10 ran the bounded exercise; Escape dismissed confirmation.", "Stopping is preserved as its own machine-readable outcome rather than being relabeled as generic completion.", "Tour · bounded work · stopped", "F5 / F10 / Escape"),
  stage("tour-four-exercise-one", "Tour 4.1 crossed one Cord", "The first two-host lesson presents hello across one Cord.", "F5 advanced to chapter four; F10 ran the planned source-to-sink transfer; Escape dismissed confirmation.", "Source and sink fragments, active Plays, typed ports, and the exact Line remain distinct and correlated.", "Tour · two hosts · Cord · Line", "F5 / F10 / Escape"),
  stage("tour-four-exercise-two", "Tour 4.2 crossed one Cord", "The second two-host lesson also presents hello across one Cord through its own exact realization.", "F3 selected the sibling exercise; F10 ran it; Escape dismissed confirmation.", "The shared application path preserves the second exercise's separate source, sink, Plan, Play, and Line identities.", "Tour · sibling Form · two hosts", "F3 / F10 / Escape"),
  stage("tour-chapter-five", "Tour chapter five visited", "The native Tour shows the complete fifth chapter page with its beginner-facing explanation.", "F5 navigated forward after completing every executable exercise in chapters one through four.", "A chapter page is retained presentation state, not fabricated execution or an unplanned Play.", "Tour · chapter · presentation", "F5"),
  stage("tour-chapter-six", "Tour chapter six visited", "The native Tour shows the complete sixth chapter page.", "F5 advanced through the same shared Tour program and native presentation implementation.", "Navigation changes the presented page while preserving the Tour application and Body identities.", "Tour · navigation · continuity", "F5"),
  stage("tour-chapter-seven", "Tour chapter seven visited", "The native Tour shows its final chapter page without an empty panel or dialog.", "F5 advanced to the final page after every earlier chapter and exercise had been visited.", "The final page is rendered from the same canonical Tour catalog used by the other hosts.", "Tour · complete catalog · native presentation", "F5"),
  stage("tour-one-exercise-one-returned", "Tour 1.1 revisited", "Returning to the first lesson preserves its HELLO result and completed state.", "F6 navigated back through every chapter; F10 exercised the already-completed action path.", "Revisiting does not invent a new result or erase the exact identities retained by the original run.", "Tour · navigation · retained completion", "F6 / F10"),
  stage("refusal-transient", "Repeated run refused", "A distinct refusal surface explains that the completed lesson cannot be run again.", "F10 requested Run after the action had become unavailable.", "The same input path produces a machine-readable refusal rather than a second Play or false success.", "Tour · refusal · unavailable action", "F10"),
  stage("refusal-dismissed", "Refusal dismissed", "The refusal is closed and the completed lesson remains unchanged.", "Escape dismissed the refusal surface.", "Closing presentation state does not erase the refusal record or mutate the completed application state.", "Tour · refusal · presentation closure", "Escape"),
  stage("tour-patchbay-open", "Tour Patchbay open", "The shared Patchbay view shows the lesson's Form and retained result.", "F11 opened Patchbay inside the same Tour workspace.", "Presentation continuity uses the shared Patchbay surface and makes no claim that another Play occurred.", "Tour · Patchbay · ports · cords", "F11"),
  stage("chooser-pointer-focused", "Chooser focused by pointer", "The Patchbay chooser transient visibly owns pointer focus.", "A primary-button report crossed the emulated USB mouse path into the chooser.", "Focus belongs to the exact transient manifestation and is distinct from hover, selection, and keyboard focus.", "Patchbay · transient · pointer focus", "Primary button"),
  stage("pointer-hover-or-focus", "Pointer hover", "Patchbay visibly marks the Gear under the pointer.", "A relative-motion report moved the QEMU USB mouse onto the Gear.", "A real emulated xHCI, USB, HID, and portable pointer path produced the correlated hover transition.", "Patchbay · Gear · pointer", "Pointer motion"),
  stage("pointer-selected", "Pointer selected", "The hit-tested Patchbay Gear is selected.", "A distinct primary-button press selected the hovered Gear.", "The record distinguishes press, selection, and later release while the screenshot documents the resulting manifestation.", "Patchbay · Gear · selection", "Primary button"),
  stage("inspector-focused", "Inspector focused", "The independent Gear inspector visibly owns pointer focus.", "Pointer motion and a primary-button press targeted the inspector after selection.", "The retained routed pointer identity reaches the exact auxiliary surface and stale manifestations remain inadmissible.", "Patchbay · inspector · routed pointer identity", "Pointer motion + primary button"),
  stage("inspector-long-text", "Inspector long text retained", "The focused inspector presents its complete bounded detail text without losing the auxiliary surface.", "The native journey retained the focused inspector after rendering a longer detail value.", "The same production presentation path carries the longer value inside its admitted surface instead of truncating, replacing, or inventing application state.", "Patchbay · inspector · bounded text", "Long detail presentation"),
  stage("inspector-closed", "Inspector closed", "The Inspector is dismissed and its auxiliary surface is gone.", "The journey activated the native Inspector Close control.", "The retained Inspector surface was removed and input focus returned to the remaining Tour workspace.", "Inspector · lifecycle · focus", "Close"),
]);
const CHECKPOINTS = JOURNEY.map(({ name }) => name);
const STAGES = new Map(JOURNEY.map((entry) => [entry.name, entry]));
const [evidenceRoot, siteRoot, commit] = process.argv.slice(2);
if (!evidenceRoot || !siteRoot || !/^[0-9a-f]{40}$/.test(commit ?? "")) {
  throw new Error("usage: stage-conduitos-pages-evidence.mjs EVIDENCE SITE 40_HEX_COMMIT");
}

const manifest = JSON.parse(await readFile(path.join(evidenceRoot, "manifest.json"), "utf8"));
if (manifest.schema !== "conduit.conduitos/visual-journey@1"
  || manifest.proof_class !== "freestanding-emulator" || manifest.status !== "complete"
  || manifest.failure !== null || manifest.context?.source_commit !== commit
  || !Array.isArray(manifest.checkpoints) || manifest.checkpoints.length !== CHECKPOINTS.length) {
  throw new Error("ConduitOS visual journey is incomplete, malformed, or belongs to another commit");
}

const entries = new Map();
for (const entry of manifest.checkpoints) {
  const name = entry?.checkpoint;
  if (!CHECKPOINTS.includes(name) || entries.has(name)
    || entry.health_refusal != null || entry.frame?.checkpoint !== name
    || entry.frame.width !== 1280 || entry.frame.height !== 800
    || entry.frame.pixel_format !== "RGBA8" || !(entry.frame.non_background_pixels > 0)
    || entry.frame.png !== `${name}.png` || !/^[0-9a-f]{64}$/.test(entry.frame.png_sha256 ?? "")) {
    throw new Error(`ConduitOS checkpoint '${name ?? "unknown"}' violates the publication contract`);
  }
  entries.set(name, entry);
}

for (const name of CHECKPOINTS) {
  if (!entries.has(name)) throw new Error(`ConduitOS visual journey omitted '${name}'`);
}

const commitRoot = path.join(siteRoot, "commits", commit, "conduitos", "x86_64");
const currentRoot = path.join(siteRoot, "current", "conduitos", "x86_64");
await rm(commitRoot, { recursive: true, force: true });
await rm(currentRoot, { recursive: true, force: true });
await mkdir(commitRoot, { recursive: true });
await mkdir(currentRoot, { recursive: true });
await cp(path.join(evidenceRoot, "manifest.json"), path.join(commitRoot, "manifest.json"));

for (const name of CHECKPOINTS) {
  const source = path.join(evidenceRoot, `${name}.png`);
  const bytes = await readFile(source);
  if (bytes.length !== entries.get(name).frame.png_bytes
    || sha256(bytes) !== entries.get(name).frame.png_sha256
    || !bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) {
    throw new Error(`ConduitOS checkpoint '${name}' PNG bytes do not match the manifest`);
  }
  await cp(source, path.join(commitRoot, `${name}.png`));
  await cp(source, path.join(currentRoot, `${name}.png`));
  await writePage(path.join(commitRoot, name, "index.html"), name, commit, "../", "../../../../../index.html", manifest.context);
  await writePage(path.join(currentRoot, name, "index.html"), name, commit, "../", "../../../../index.html", manifest.context);
}
await writeIndex(path.join(commitRoot, "index.html"), commit, "../../../../index.html", "manifest.json");
await writeIndex(path.join(currentRoot, "index.html"), commit, "../../../index.html", `../../../commits/${commit}/conduitos/x86_64/manifest.json`);

async function writeIndex(destination, exactCommit, home, manifestLink) {
  const sections = JOURNEY.map((entry, index) => `<article id="${entry.name}"><p class="step">Checkpoint ${index + 1} of ${JOURNEY.length} · ${entry.action}</p><h2><a href="${entry.name}/">${entry.title}</a></h2><a href="${entry.name}/"><img src="${entry.name}.png" alt="${entry.seeing}"></a>${narrative(entry)}</article>`).join("\n");
  await html(destination, "ConduitOS visual journey", `<nav><a href="${home}">Gallery home</a></nav><h1>ConduitOS visual journey</h1><p class="lede">Follow one ordinary QEMU session from boot, through Body and Play lifecycle, across a hot-plugged Line, and into Tour and Patchbay interaction.</p><p><strong>FREESTANDING EMULATOR EVIDENCE, NOT PHYSICAL HARDWARE EVIDENCE.</strong> The guest records and acceptance assertions prove the named behavior; these real captured pixels make that proof human-inspectable.</p><p>Exact accepted commit: <code>${exactCommit}</code> · <a href="${manifestLink}">Correlated journey manifest</a></p>${sections}`);
}

async function writePage(destination, name, exactCommit, imageRoot, home, context) {
  const entry = STAGES.get(name);
  const escapedContext = escapeHtml(JSON.stringify(context, null, 2));
  await html(destination, entry.title, `<nav><a href="${home}">Gallery home</a> · <a href="../">ConduitOS journey</a></nav><h1>${entry.title}</h1><p class="step">Input or transition: ${entry.action}</p><img src="${imageRoot}${name}.png" alt="${entry.seeing}">${narrative(entry)}<p><strong>FREESTANDING EMULATOR EVIDENCE, NOT PHYSICAL HARDWARE EVIDENCE.</strong></p><h2>Exact correlation</h2><p>Accepted commit: <code>${exactCommit}</code></p><pre>${escapedContext}</pre>`);
}

async function html(destination, title, body) {
  await mkdir(path.dirname(destination), { recursive: true });
  await writeFile(destination, `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title}</title><style>body{margin:2rem auto;max-width:82rem;padding:0 1rem;font:16px/1.55 system-ui,sans-serif;background:#101714;color:#e8f5ed}a{color:#70e0aa}code,pre{overflow-wrap:anywhere}pre{white-space:pre-wrap}.lede{font-size:1.2rem;max-width:65rem}.step{color:#a8cab8;font-weight:700;letter-spacing:.03em}article{margin:4rem 0;padding-top:1rem;border-top:1px solid #466455}dl{display:grid;grid-template-columns:max-content minmax(0,1fr);gap:.5rem 1rem}dt{font-weight:700}dd{margin:0}img{display:block;width:100%;height:auto;margin:1rem 0 1.5rem;border:1px solid #466455}</style></head><body>${body}</body></html>`);
}

function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function stage(name, title, seeing, happened, proving, concepts, action) { return Object.freeze({ name, title, seeing, happened, proving, concepts, action }); }
function narrative(entry) { return `<dl><dt>What you see</dt><dd>${entry.seeing}</dd><dt>What just happened</dt><dd>${entry.happened}</dd><dt>What the harness proves</dt><dd>${entry.proving}</dd><dt>Concepts in view</dt><dd>${entry.concepts}</dd></dl>`; }
function escapeHtml(value) { return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;"); }
