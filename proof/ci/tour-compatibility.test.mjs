import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runInNewContext } from "node:vm";
import test from "node:test";
import { stageLegacyTourRoutes } from "../../products/tour/tools/stage-legacy-routes.mjs";
import { openTourReadingState } from "../../products/tour/browser/tour-state.mjs";
import {
  COMPACT_PATCHBAY_CONTRACT,
  compactPatchbaySnapshot,
} from "../../products/tour/browser/tour-compact-patchbay.mjs";

test("published Book routes retire into the Body while retaining query and fragment", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "conduit-tour-compat-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await stageLegacyTourRoutes(root);
  for (const route of ["", "meet-one-gear", "same-face-different-implementation", "faces-backs-and-implementation"]) {
    const html = await readFile(join(root, "book", route, "index.html"), "utf8");
    const script = html.match(/<script>(.*?)<\/script>/s)[1];
    const old = new URL(`https://example.test/conduit/book/${route ? `${route}/` : ""}?from=legacy#source`);
    let redirected;
    runInNewContext(script, { location: {
      search: old.search, hash: old.hash,
      replace: (target) => { redirected = new URL(target, old).href; },
    } });
    const expected = new URL(route ? "../../workspace/" : "../workspace/", old);
    expected.search = old.search;
    expected.hash = old.hash;
    assert.equal(redirected, expected.href);
  }
});

test("Tour retains the saved-state namespace and upgrades a bounded legacy draft", async () => {
  const manifest = JSON.parse(await readFile("products/tour/browser/tour.application.template.json", "utf8"));
  assert.equal(manifest.application_id, "conduit.application/tour");
  assert.deepEqual(manifest.state_compatibility, { identity: "conduit.application/book-reading-state", version: 1 });
  const legacy = { schema: "conduit.book/reading-state@1", drafts: [["canonical-form:hello", "form hello {}"]], expandedBacks: ["hello/morse"] };
  const writes = [];
  const reading = await openTourReadingState({
    readJson: async (key) => key === "reading-state" ? legacy : null,
    writeJson: async (key, value) => writes.push({ key, value }),
  });
  await reading.flush();
  assert.equal(reading.drafts.get("canonical-form:hello"), "form hello {}");
  assert.ok(reading.expandedBacks.has("hello/morse"));
  assert.deepEqual(writes, [{ key: "reading-state", value: { ...legacy, schema: "conduit.tour/reading-state@1" } }]);
});

test("legacy migration refuses malformed and over-capacity state without rewriting it", async () => {
  const base = { schema: "conduit.book/reading-state@1", drafts: [], expandedBacks: [] };
  for (const state of [
    { ...base, schema: "unknown" },
    { ...base, drafts: Array.from({ length: 33 }, (_, i) => [`draft-${i}`, "form hello {}"]) },
    { ...base, drafts: [["draft", "x".repeat(4097)]] },
  ]) {
    let writes = 0;
    await assert.rejects(openTourReadingState({
      readJson: async (key) => key === "reading-state" ? state : null,
      writeJson: async () => { writes += 1; },
    }));
    assert.equal(writes, 0);
  }
});

test("compact Tour Patchbay is an inspection-only checked Form projection", () => {
  const projection = {
    sequence: 7,
    source_proposal_id: "proposal/7",
    source_document_id: "source/7",
    checked_form_id: "checked/7",
    visible_expanded_form_id: "expanded/7",
    diagnostics: [],
    gears: [{
      gear_id: "gear/upper",
      kind_id: "text/upper",
      inputs: [{ port_id: "text", info_kind: "text/utf8", temporal: "state" }],
      outputs: [{ port_id: "text", info_kind: "text/utf8", temporal: "event" }],
    }],
    cords: [],
    realization_gears: [],
    realization_cords: [],
  };
  const snapshot = compactPatchbaySnapshot(projection);
  assert.equal(snapshot.contract, COMPACT_PATCHBAY_CONTRACT);
  assert.equal(snapshot.contract.operationMode, "inspection-only");
  assert.deepEqual(snapshot.contract.supportedOperations, ["open-back"]);
  assert.deepEqual(snapshot.presentation.actions, []);
  assert.deepEqual(snapshot.presentation.basis, {
    source_document_id: "source/7",
    checked_form_id: "checked/7",
  });
  for (const invented of [
    "body_id", "plan_id", "active_play_id", "sign_id", "authority", "delivery",
  ]) {
    assert.equal(JSON.stringify(snapshot).includes(invented), false);
  }
  assert.deepEqual(
    snapshot.presentation.subjects
      .filter(({ role }) => role === "Port")
      .map(({ identity }) => identity),
    ["gear/upper.receiving:text", "gear/upper.emitting:text"],
  );
});
