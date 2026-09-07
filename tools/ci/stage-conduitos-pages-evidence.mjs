#!/usr/bin/env node

import { createHash } from "node:crypto";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const CHECKPOINTS = [
  "front-door-ready", "tour-opened", "tour-result-visible", "tour-patchbay-open",
  "pointer-hover-or-focus", "pointer-selected",
  "form-opened", "born-lulled", "awake",
  "planned", "playing", "result-visible", "lulled",
  "usb-line-current", "peer-attached", "line-value-visible", "line-lost",
];
const DESCRIPTIONS = Object.freeze({
  "front-door-ready": "The ordinary ConduitOS front door after the exact Crèche artifact has booted and exposed its bounded product entrance.",
  "tour-opened": "Tour opened from the ordinary ConduitOS product surface after the Body journey returned to Lulled, through emulated xHCI, USB HID, and the admitted portable keyboard path; the canonical meet-one-gear lesson is ready but has not run.",
  "tour-result-visible": "The canonical Tour Form has run through the production kernel and the visible HELLO result is correlated with exact source, checked Form, expanded Form, Plan, and active Play identities.",
  "tour-patchbay-open": "The same Tour workspace has opened its shared Patchbay surface while retaining the canonical specimen and visible result; this frame documents presentation continuity rather than a new Play.",
  "pointer-hover-or-focus": "A real emulated USB boot-pointer report crossed xHCI, USB, HID, and the portable pointer contract before the shared Patchbay geometry visibly marked the hovered Gear.",
  "pointer-selected": "A distinct primary-button press crossed the same admitted USB pointer path and visibly selected the hit-tested Patchbay Gear; the screenshot documents manifestation while the correlated guest record proves the semantic transition.",
  "form-opened": "The selected product Form opened in the same guest, before Body birth or Play execution.",
  "born-lulled": "The newly born Body retained in its Lulled state, with no active execution claimed.",
  "awake": "The same Body after Wake, ready for planning while preserving the boot and manifestation context.",
  "planned": "The product after an exact immutable Plan has been admitted, before that Plan becomes an active Play.",
  "playing": "The admitted Plan running as an active Play through the ordinary ConduitOS product path.",
  "result-visible": "The visible product result after guest semantic records report the correlated runtime outcome.",
  "lulled": "The completed journey returned to Lulled while retaining the same Body and exact journey evidence.",
  "usb-line-current": "The same ordinary guest visibly reports one current bounded Line backed by QEMU's separately pinned FTDI USB function; the correlated guest record, rather than this raster, establishes controller, device, interface, endpoint, Line, binding, Host, and Boot facts.",
  "peer-attached": "The host-side harness peer has completed the canonical bounded Hello/Ready session over bytes that crossed the emulated FTDI USB device. Connectivity remains explicitly pre-admission and does not imply Body membership.",
  "line-value-visible": "The guest visibly manifests HELLO USB LINE after the canonical session delivered the bounded value across the emulated USB carrier with exact Line, session, Plan, Host, Boot, and unchanged Body correlation.",
  "line-lost": "After QMP removes the separately pinned FTDI USB function, the ordinary guest visibly reports Line loss; correlated records establish device loss and stale-session refusal without changing Body identity.",
});
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
  const links = CHECKPOINTS.map((name) => `<li><a href="${name}/">${label(name)}</a> — ${DESCRIPTIONS[name]}</li>`).join("\n");
  await html(destination, "ConduitOS visual journey", `<nav><a href="${home}">Gallery home</a></nav><h1>ConduitOS visual journey</h1><p><strong>FREESTANDING EMULATOR EVIDENCE, NOT PHYSICAL HARDWARE EVIDENCE.</strong></p><p>Exact accepted commit: <code>${exactCommit}</code></p><ul>${links}</ul><p><a href="${manifestLink}">Correlated journey manifest</a></p>`);
}

async function writePage(destination, name, exactCommit, imageRoot, home, context) {
  const escapedContext = escapeHtml(JSON.stringify(context, null, 2));
  await html(destination, label(name), `<nav><a href="${home}">Gallery home</a> · <a href="../">ConduitOS journey</a></nav><h1>${label(name)}</h1><p>${DESCRIPTIONS[name]}</p><p><strong>FREESTANDING EMULATOR EVIDENCE, NOT PHYSICAL HARDWARE EVIDENCE.</strong></p><img src="${imageRoot}${name}.png" alt="ConduitOS ${label(name)} at accepted commit ${exactCommit}"><h2>Exact correlation</h2><pre>${escapedContext}</pre>`);
}

async function html(destination, title, body) {
  await mkdir(path.dirname(destination), { recursive: true });
  await writeFile(destination, `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title}</title><style>body{margin:2rem auto;max-width:82rem;padding:0 1rem;font:16px/1.5 system-ui,sans-serif;background:#101714;color:#e8f5ed}a{color:#70e0aa}code,pre{overflow-wrap:anywhere}pre{white-space:pre-wrap}img{display:block;max-width:100%;height:auto;border:1px solid #466455}</style></head><body>${body}</body></html>`);
}

function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function label(name) { return name.split("-").map((part) => `${part[0].toUpperCase()}${part.slice(1)}`).join(" "); }
function escapeHtml(value) { return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;"); }
