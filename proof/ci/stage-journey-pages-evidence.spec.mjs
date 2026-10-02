import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const script = "tools/ci/stage-journey-pages-evidence.mjs";
const commit = "0123456789abcdef0123456789abcdef01234567";
const inventories = new Map([
  ["one-form-two-fronts", ["index.html", "manifest.json", "native.png", "native.json", "browser.png", "browser.json"]],
  ["little-life", ["index.html", "manifest.json", "t000.png", "t001.png", "t008.png", "t032.png", "presentation.txt", "execution.json"]],
]);
const verticals = [
  ["field-station-clock", "accepted", "08c5ad83e8bebe9dc195c5d77772ffb8ee67aa30"],
  ["durable-notebook", "accepted", "75bc7d7b535b8c7df8ad17a5fc27b33c3c3812a0"],
  ["bare-metal-to-show", "accepted", "36c9be63f0d3dd6e7a8bfc09e7afe1a06d6d96cd"],
  ["pocket-theremin", "accepted", "319ab3121979da2e7eefdb0c622e74e9baa29386"],
  ["two-ollamas", "accepted", "24a33926c582aa6035aca88e9a1252083ce2dbce"],
  ["three-bodies", "accepted", "8c6f4a8bea743b733fa9de46aaeb6c6f119e1faa"],
];

test("stages only the exact sealed sibling gallery behind the authored entrance", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "conduit-journey-stage-"));
  const gallery = path.join(root, "gallery");
  const site = path.join(root, "site");
  fixture(gallery);
  mkdirSync(site);
  const homepage = "<!doctype html><body><a href=\"/conduit/journeys/\">Journeys</a></body>";
  writeFileSync(path.join(site, "index.html"), homepage);

  execFileSync("node", [script, gallery, site, commit]);

  assert.equal(readFileSync(path.join(site, "index.html"), "utf8"), homepage);
  assert.equal(
    readFileSync(path.join(site, "journeys/current/little-life/t032.png"), "utf8"),
    "little-life:t032.png",
  );
  assert.equal(
    readFileSync(path.join(site, `journeys/commits/${commit}/one-form-two-fronts/native.png`), "utf8"),
    "one-form-two-fronts:native.png",
  );
  rmSync(root, { recursive: true, force: true });
});

test("stages the nonempty verified subset a source commit can still reproduce", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "conduit-journey-stage-subset-"));
  const gallery = path.join(root, "gallery");
  const site = path.join(root, "site");
  fixture(gallery, new Map([["little-life", inventories.get("little-life")]]));
  mkdirSync(site);
  writeFileSync(path.join(site, "index.html"), "<!doctype html><body><a href=\"/conduit/journeys/\">Journeys</a></body>");

  execFileSync("node", [script, gallery, site, commit]);

  assert.equal(
    readFileSync(path.join(site, "journeys/current/little-life/t032.png"), "utf8"),
    "little-life:t032.png",
  );
  rmSync(root, { recursive: true, force: true });
});

test("refuses a Pages root without an authored Journeys entrance", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "conduit-journey-stage-no-entrance-"));
  const gallery = path.join(root, "gallery");
  const site = path.join(root, "site");
  fixture(gallery);
  mkdirSync(site);
  writeFileSync(path.join(site, "index.html"), "<!doctype html><body><h1>Conduit</h1></body>");

  assert.throws(
    () => execFileSync("node", [script, gallery, site, commit], { stdio: "pipe" }),
    /Command failed/,
  );
  assert.equal(readFileSync(path.join(site, "index.html"), "utf8"), "<!doctype html><body><h1>Conduit</h1></body>");
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

function fixture(root, journeyInventories = inventories) {
  mkdirSync(root, { recursive: true });
  writeFileSync(path.join(root, ".nojekyll"), "");
  writeFileSync(path.join(root, "index.html"), "<!doctype html><body>journeys</body>");
  writeFileSync(path.join(root, "gallery.json"), `${JSON.stringify({
    schema: "conduit.visual-evidence-gallery/v1",
    current_commit: commit,
    retention_commits: 32,
    commits: [commit],
  })}\n`);
  writeFileSync(path.join(root, "catalogue.json"), `${JSON.stringify({
    schema: "conduit.vertical-journey-catalogue/v1",
    publication_commit: commit,
    verticals: verticals.map(([slug, state, accepted_source]) => ({
      slug,
      state,
      ...(accepted_source ? { accepted_source } : {}),
    })),
  })}\n`);
  mkdirSync(path.join(root, "verticals"));
  writeFileSync(path.join(root, "verticals/index.html"), "<!doctype html><body>verticals</body>");
  for (const [slug] of verticals) {
    mkdirSync(path.join(root, "verticals", slug));
    writeFileSync(path.join(root, "verticals", slug, "index.html"), `<!doctype html><body>${slug}</body>`);
  }
  for (const [journey, files] of journeyInventories) {
    for (const prefix of [path.join("current", journey), path.join("commits", commit, journey)]) {
      const directory = path.join(root, prefix);
      mkdirSync(directory, { recursive: true });
      for (const file of files) writeFileSync(path.join(directory, file), `${journey}:${file}`);
    }
  }
}
