import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const checkpoints = [
  "front-door-ready", "form-opened", "born-lulled", "awake", "planned", "playing",
  "result-visible", "lulled", "usb-line-current", "peer-attached", "line-value-visible",
  "line-lost", "tour-opened", "tour-result-visible", "tour-patchbay-open",
  "pointer-hover-or-focus", "pointer-selected",
];
const commit = "1".repeat(40);
const png = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10, 1]);

test("ConduitOS journey publisher renders a complete narrated sequence", () => {
  const fixture = makeFixture();
  try {
    const result = publish(fixture);
    assert.equal(result.status, 0, result.stderr);
    const page = readFileSync(path.join(fixture.site, "current/conduitos/x86_64/index.html"), "utf8");
    assert.match(page, /What you see/);
    assert.match(page, /What just happened/);
    assert.match(page, /What the harness proves/);
    assert.match(page, /Concepts in view/);
    assert.match(page, /src="front-door-ready\.png"/);
    assert.ok(page.indexOf("Form opened") < page.indexOf("Tour opened"), "walkthrough is not in journey order");
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("ConduitOS journey publisher refuses checkpoint drift", () => {
  const fixture = makeFixture();
  try {
    const manifestPath = path.join(fixture.evidence, "manifest.json");
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    manifest.checkpoints[1].checkpoint = "form-renamed";
    writeFileSync(manifestPath, JSON.stringify(manifest));
    const result = publish(fixture);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /violates the publication contract/);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

function makeFixture() {
  const root = mkdtempSync(path.join(tmpdir(), "conduit-journey-docs-"));
  const evidence = path.join(root, "evidence");
  const site = path.join(root, "site");
  mkdirSync(evidence);
  mkdirSync(site);
  const digest = createHash("sha256").update(png).digest("hex");
  const entries = checkpoints.map((checkpoint, index) => {
    writeFileSync(path.join(evidence, `${checkpoint}.png`), png);
    return { index, checkpoint, health_refusal: null, frame: {
      checkpoint, width: 1280, height: 800, pixel_format: "RGBA8",
      non_background_pixels: 1, png: `${checkpoint}.png`, png_bytes: png.length,
      png_sha256: digest,
    } };
  });
  writeFileSync(path.join(evidence, "manifest.json"), JSON.stringify({
    schema: "conduit.conduitos/visual-journey@1", proof_class: "freestanding-emulator",
    status: "complete", failure: null, context: { source_commit: commit }, checkpoints: entries,
  }));
  return { root, evidence, site };
}

function publish({ evidence, site }) {
  return spawnSync(process.execPath, [
    "tools/ci/stage-conduitos-pages-evidence.mjs", evidence, site, commit,
  ], { encoding: "utf8" });
}
