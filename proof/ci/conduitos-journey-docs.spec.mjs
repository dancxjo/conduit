import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const checkpoints = [
  "front-door-ready", "body-awake", "home-prompt", "home-forms", "home-play-observed", "home-patchbay-open",
  "home-returned",
  "host-current-offers", "quiescent-awaiting-input",
  "input-continued", "patchbay-current-canvas", "patchbay-edit-requested", "patchbay-presenters-replanned",
  "resident-tour-result", "memory-listening", "canvas-retained", "memory-cleared",
  "patchbay-current-canvas-returned", "memory-retained", "lulled", "usb-line-current", "peer-attached", "line-value-visible",
  "line-lost", "tour-opened", "tour-result-visible", "confirmation-transient",
  "confirmation-dismissed", "tour-one-exercise-two", "tour-one-exercise-three",
  "tour-two-exercise-one", "tour-three-exercise-one", "tour-four-exercise-one",
  "tour-four-exercise-two", "tour-chapter-five", "tour-chapter-six", "tour-chapter-seven",
  "tour-one-exercise-one-returned", "refusal-transient", "refusal-dismissed",
  "tour-patchbay-open", "chooser-pointer-focused", "pointer-hover-or-focus",
  "pointer-selected", "inspector-focused", "inspector-long-text", "inspector-closed",
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
    assert.match(page, /What happened/);
    assert.match(page, /What Conduit established/);
    assert.match(page, /Concepts in view/);
    assert.match(page, /Focused artifact and exact correlation/);
    assert.match(page, /What this proves/);
    assert.match(page, /What it does not prove/);
    assert.match(page, /cargo xtask conduitos journey-proof/);
    assert.match(page, /src="front-door-ready\.png"/);
    assert.match(page, /src="host-current-offers\.png"/);
    assert.match(page, /src="home-prompt\.png"/);
    assert.match(page, /src="home-forms\.png"/);
    assert.match(page, /src="home-patchbay-open\.png"/);
    assert.match(page, /src="home-returned\.png"/);
    assert.match(page, /src="quiescent-awaiting-input\.png"/);
    assert.match(page, /src="input-continued\.png"/);
    assert.match(page, /structural drain remains distinct from semantic completion/);
    assert.match(page, /src="patchbay-current-canvas\.png"/);
    assert.match(page, /src="patchbay-edit-requested\.png"/);
    assert.match(page, /src="patchbay-presenters-replanned\.png"/);
    assert.match(page, /src="resident-tour-result\.png"/);
    assert.match(page, /src="memory-listening\.png"/);
    assert.match(page, /src="canvas-retained\.png"/);
    assert.match(page, /src="memory-cleared\.png"/);
    assert.match(page, /src="patchbay-current-canvas-returned\.png"/);
    assert.match(page, /src="memory-retained\.png"/);
    assert.match(page, /src="confirmation-transient\.png"/);
    assert.match(page, /src="tour-one-exercise-two\.png"/);
    assert.match(page, /src="tour-chapter-seven\.png"/);
    assert.match(page, /src="tour-one-exercise-one-returned\.png"/);
    assert.match(page, /src="inspector-focused\.png"/);
    assert.match(page, /src="inspector-long-text\.png"/);
    assert.ok(page.indexOf("Crèche ready") < page.indexOf("Tour opened"), "walkthrough is not in journey order");
    const journeys = readFileSync(path.join(fixture.site, "journeys/index.html"), "utf8");
    assert.match(journeys, /A computer is born/);
    assert.match(journeys, /href="\.\.\/current\/conduitos\/x86_64\/"/);
    assert.doesNotMatch(journeys, /conduit-conduitos-journey-card/);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});

test("ConduitOS journey publisher refuses checkpoint drift", () => {
  const fixture = makeFixture();
  try {
    const manifestPath = path.join(fixture.evidence, "manifest.json");
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    manifest.checkpoints[1].checkpoint = "creche-renamed";
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
  mkdirSync(path.join(site, "journeys"));
  writeFileSync(
    path.join(site, "journeys/index.html"),
    "<!doctype html><body><div class=\"cards\"><!-- conduit-conduitos-journey-card@1 --></div></body>",
  );
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
