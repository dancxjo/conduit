import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runInNewContext } from "node:vm";
import test from "node:test";
import { stageLegacyTourRoutes } from "../../products/tour/tools/stage-legacy-routes.mjs";
import { openTourReadingState } from "../../products/tour/browser/tour-state.mjs";

test("published Book routes retain their exact Tour destination, query and fragment", async (t) => {
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
    assert.equal(redirected, old.href.replace("/book/", "/tour/"));
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
