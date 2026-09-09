#!/usr/bin/env node

import { createHash } from "node:crypto";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const JOURNEY = Object.freeze([
  stage("front-door-ready", "Front door ready", "The empty ConduitOS product front door is ready for a Form.", "The exact Crèche-produced image booted to its ordinary bounded entrance.", "Boot reached the named ready Sign and the framebuffer is healthy and correlated to this image and Boot.", "Boot · Host · Form", "Boot"),
  stage("form-opened", "Form opened", "The selected text Form is open, but no Body or execution exists yet.", "Return opened the Form from the front door.", "The emulated USB keyboard action reached the product and source, checked, and expanded Form identities remain distinct.", "Form · source document", "Return"),
  stage("born-lulled", "Body born, Lulled", "A new Body now owns the Form and is resting without an active Play.", "F3 asked the product to birth the Body.", "Birth produced a stable Body identity without silently waking or executing it.", "Body · Lull", "F3"),
  stage("awake", "Body awake", "The same Body is awake and available for planning.", "F4 sent Wake to the Lulled Body.", "Wake changed lifecycle state while retaining the exact Body, Host, and Boot correlation.", "Body · Wake", "F4"),
  stage("planned", "Plan admitted", "An exact Plan is visible, admitted but not yet active.", "F5 planned the current Form against the Host's offered implementations and finite resources.", "Planning selected exact implementations, ports, and bounds before Play start.", "Plan · ports · implementations", "F5"),
  stage("playing", "Play active", "The admitted Plan is executing as an active Play.", "F6 started the Plan.", "The production kernel entered Play with the previously admitted identities and bounds.", "Plan · Play · ports", "F6"),
  stage("result-visible", "Result visible", "The presentation shows the uppercase result produced by the running Form.", "The semantic A input crossed the admitted keyboard path and flowed through the Form.", "The visible value is correlated with the port-specific kernel result; pixels document it but do not establish it alone.", "Form · Play · presentation", "A"),
  stage("lulled", "Body Lulled", "The completed Body is retained and no Play is active.", "F7 lulled the Body after the result.", "Execution stopped without erasing Body or journey identity.", "Body · Lull · Play", "F7"),
  stage("usb-line-current", "USB Line current", "The product reports one current bounded Line backed by the emulated FTDI USB function.", "F12 opened the Line view after QEMU had supplied the separate USB function.", "Guest records establish the exact controller, endpoints, binding, Host, and Boot; the screenshot only shows their presentation.", "Cord · Line · ports", "F12"),
  stage("peer-attached", "Peer attached", "The Line view reports that the harness peer completed its bounded greeting.", "The peer exchanged the canonical Hello and Ready bytes over the emulated USB device.", "Bytes crossed the real emulated carrier, while the record keeps connectivity distinct from Body membership or trust.", "Line · peer · authority", "Hello / Ready"),
  stage("line-value-visible", "Line value visible", "HELLO USB LINE is manifested in the guest.", "The peer delivered one bounded value and acknowledged the session.", "Delivery is correlated to distinct Line, binding, Plan, active Play, Host, Boot, and unchanged Body identities.", "Line · value · Play", "Value + acknowledgment"),
  stage("line-lost", "Line lost", "The product visibly reports that the USB Line has disappeared.", "The harness used QMP to remove the separately pinned FTDI USB function.", "Removal is detected, the stale session is refused, and Body identity remains unchanged rather than turning loss into success.", "Line loss · stale identity", "QMP device removal"),
  stage("tour-opened", "Tour opened", "The canonical meet-one-gear lesson is open and has not run yet.", "F9 opened Tour from the ordinary product after the Body returned to Lulled.", "The same emulated keyboard and portable action path reaches the shared Tour application without creating a second runtime.", "Tour · Form · Gear", "F9"),
  stage("tour-result-visible", "Tour result visible", "Tour displays HELLO from its canonical Form.", "F10 ran the lesson.", "The production kernel result remains correlated to exact source, checked Form, expanded Form, Plan, and active Play identities.", "Tour · Form · Plan · Play", "F10"),
  stage("confirmation-transient", "Confirmation shown", "A separate confirmation surface is visible above the completed Tour result.", "The successful run manifested its bounded confirmation transient.", "The transient has its own surface identity and relationship without replacing the result or becoming application truth.", "Tour · transient · presentation", "Run completion"),
  stage("confirmation-dismissed", "Confirmation dismissed", "The confirmation is gone and the retained Tour result is visible again.", "Escape dismissed the exact confirmation surface.", "Dismissal closes only the transient while preserving the underlying result and workspace identities.", "Tour · transient closure · result", "Escape"),
  stage("refusal-transient", "Repeated run refused", "A distinct refusal surface explains that the completed lesson cannot be run again.", "F10 requested Run after the action had become unavailable.", "The same input path produces a machine-readable refusal rather than a second Play or false success.", "Tour · refusal · unavailable action", "F10"),
  stage("refusal-dismissed", "Refusal dismissed", "The refusal is closed and the completed lesson remains unchanged.", "Escape dismissed the refusal surface.", "Closing presentation state does not erase the refusal record or mutate the completed application state.", "Tour · refusal · presentation closure", "Escape"),
  stage("tour-patchbay-open", "Tour Patchbay open", "The shared Patchbay view shows the lesson's Form and retained result.", "F11 opened Patchbay inside the same Tour workspace.", "Presentation continuity uses the shared Patchbay surface and makes no claim that another Play occurred.", "Tour · Patchbay · ports · cords", "F11"),
  stage("chooser-pointer-focused", "Chooser focused by pointer", "The Patchbay chooser transient visibly owns pointer focus.", "A primary-button report crossed the emulated USB mouse path into the chooser.", "Focus belongs to the exact transient manifestation and is distinct from hover, selection, and keyboard focus.", "Patchbay · transient · pointer focus", "Primary button"),
  stage("pointer-hover-or-focus", "Pointer hover", "Patchbay visibly marks the Gear under the pointer.", "A relative-motion report moved the QEMU USB mouse onto the Gear.", "A real emulated xHCI, USB, HID, and portable pointer path produced the correlated hover transition.", "Patchbay · Gear · pointer", "Pointer motion"),
  stage("pointer-selected", "Pointer selected", "The hit-tested Patchbay Gear is selected.", "A distinct primary-button press selected the hovered Gear.", "The record distinguishes press, selection, and later release while the screenshot documents the resulting manifestation.", "Patchbay · Gear · selection", "Primary button"),
  stage("inspector-focused", "Inspector focused", "The independent Gear inspector visibly owns pointer focus.", "Pointer motion and a primary-button press targeted the inspector after selection.", "The retained routed pointer identity reaches the exact auxiliary surface and stale manifestations remain inadmissible.", "Patchbay · inspector · routed pointer identity", "Pointer motion + primary button"),
  stage("inspector-long-text", "Inspector long text retained", "The focused inspector presents its complete bounded detail text without losing the auxiliary surface.", "The native journey retained the focused inspector after rendering a longer detail value.", "The same production presentation path carries the longer value inside its admitted surface instead of truncating, replacing, or inventing application state.", "Patchbay · inspector · bounded text", "Long detail presentation"),
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
