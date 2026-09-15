import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const script = "tools/ci/stage-journey-pages-evidence.mjs";
const commit = "0123456789abcdef0123456789abcdef01234567";
const inventories = new Map([
  ["one-form-two-faces", ["index.html", "manifest.json", "native.png", "native.json", "browser.png", "browser.json"]],
  ["little-life", ["index.html", "manifest.json", "t000.png", "t001.png", "t008.png", "t032.png", "presentation.txt", "execution.json"]],
]);

test("stages only the exact sealed sibling gallery and links it from Pages", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "conduit-journey-stage-"));
  const gallery = path.join(root, "gallery");
  const site = path.join(root, "site");
  fixture(gallery);
  mkdirSync(site);
  writeFileSync(path.join(site, "index.html"), "<!doctype html><body><h1>Conduit</h1></body>");

  execFileSync("node", [script, gallery, site, commit]);

  assert.match(readFileSync(path.join(site, "index.html"), "utf8"), /href="journeys\/"/);
  assert.equal(
    readFileSync(path.join(site, "journeys/current/little-life/t032.png"), "utf8"),
    "little-life:t032.png",
  );
  assert.equal(
    readFileSync(path.join(site, `journeys/commits/${commit}/one-form-two-faces/native.png`), "utf8"),
    "one-form-two-faces:native.png",
  );
  rmSync(root, { recursive: true, force: true });
});

test("refuses an undeclared sibling gallery file", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "conduit-journey-stage-refusal-"));
  const gallery = path.join(root, "gallery");
  const site = path.join(root, "site");
  fixture(gallery);
  writeFileSync(path.join(gallery, "current/little-life/extra.txt"), "undeclared");
  mkdirSync(site);
  writeFileSync(path.join(site, "index.html"), "<!doctype html><body></body>");

  assert.throws(
    () => execFileSync("node", [script, gallery, site, commit], { stdio: "pipe" }),
    /Command failed/,
  );
  assert.equal(readFileSync(path.join(site, "index.html"), "utf8"), "<!doctype html><body></body>");
  rmSync(root, { recursive: true, force: true });
});

function fixture(root) {
  mkdirSync(root, { recursive: true });
  writeFileSync(path.join(root, ".nojekyll"), "");
  writeFileSync(path.join(root, "index.html"), "<!doctype html><body>journeys</body>");
  writeFileSync(path.join(root, "gallery.json"), `${JSON.stringify({
    schema: "conduit.visual-evidence-gallery/v1",
    current_commit: commit,
    retention_commits: 32,
    commits: [commit],
  })}\n`);
  for (const [journey, files] of inventories) {
    for (const prefix of [path.join("current", journey), path.join("commits", commit, journey)]) {
      const directory = path.join(root, prefix);
      mkdirSync(directory, { recursive: true });
      for (const file of files) writeFileSync(path.join(directory, file), `${journey}:${file}`);
    }
  }
}
