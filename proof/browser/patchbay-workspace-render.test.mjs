import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { chromium } from "@playwright/test";

const root = new URL("../../", import.meta.url);

test("pinned Chromium stores, switches, reloads and refuses future portable layouts in real Flow", async () => {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1366, height: 768 }, reducedMotion: "reduce" });
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.route("http://workspace.test/**", async (route) => {
      const path = new URL(route.request().url()).pathname.slice(1);
      if (!path) {
        await route.fulfill({ contentType: "text/html", body: `<!doctype html>
          <link rel="stylesheet" href="/targets/browser/host/assets/vendor/react-flow/react-flow.css">
          <link rel="stylesheet" href="/plots/patchbay/workbench/browser/flow.css">
          <script src="/targets/browser/host/assets/vendor/react-flow/react.min.js"></script>
          <script src="/targets/browser/host/assets/vendor/react-flow/react-dom.min.js"></script>
          <script src="/targets/browser/host/assets/vendor/react-flow/react-flow.min.js"></script>
          <div id="flow-root" style="width:1300px;height:700px"></div>` });
      } else {
        await route.fulfill({ contentType: path.endsWith(".css") ? "text/css" : "text/javascript", body: await readFile(new URL(path, root)) });
      }
    });
    await page.goto("http://workspace.test/");
    const fixture = JSON.parse(await readFile(new URL("proof/browser/fixtures/patchbay-workspace.json", root), "utf8"));
    const initialize = async ({ fixture, stored = [] }) => {
      window.flow = await import("/plots/patchbay/workbench/browser/flow.js");
      const subjects = fixture.layouts[0].positions.map((item, i) => ({ identity: `wrapper-${i}`, role: "Gear", name: item.subject }));
      const property = (subject, name, text) => ({ subject, name, value: { Text: text } });
      const properties = subjects.map((subject, i) => property(subject.identity, "semantic-id", fixture.layouts[0].positions[i].subject));
      const relationships = [];
      fixture.layouts[0].routes.forEach((cord, i) => {
        const source = `out-${i}`, sink = `in-${i + 1}`, identity = `cord-wrapper-${i}`;
        subjects.push({ identity: source, role: "Port", name: source, label: "Out" }, { identity: sink, role: "Port", name: sink, label: "In" }, { identity, role: "Cord", name: cord.subject });
        properties.push(property(source, "semantic-id", `port/${source}`), property(sink, "semantic-id", `port/${sink}`),
          property(source, "direction", "emitting"), property(sink, "direction", "receiving"),
          property(identity, "semantic-id", cord.subject), property(identity, "source-port", `port/${source}`), property(identity, "sink-port", `port/${sink}`));
        relationships.push({ source: `wrapper-${i}`, target: source, kind: "Contains" }, { source: `wrapper-${i + 1}`, target: sink, kind: "Contains" });
      });
      window.snapshot = { authoring: { ...fixture.basis, connection_candidates: [] }, presentation: { identity: "presentation/exact", revision: 1, basis: fixture.basis, subjects, properties, relationships, text: [], actions: [] }, interaction: { revision: 1, selected_subject: null } };
      window.before = JSON.stringify(window.snapshot);
      window.store = new Map(stored);
      window.validations = 0;
      flow.configureFlowWorkspaceValidator(async (document) => {
        (await import("/plots/patchbay/workbench/browser/flow-workspace.js")).validateWorkspace(document);
        window.validations++;
      });
      flow.configureFlowStorage({
        schema: "conduit.browser/application-storage@1",
        readJson: async (key) => window.store.get(key) ?? null,
        writeJson: async (key, value) => { window.store.set(key, value); },
        deleteJson: async (key) => { window.store.delete(key); },
      });
      window.connectionStarts = [];
      window.handlers = { lens: "plan", onSelect() {}, onClear() {}, onConnect() {},
        onConnectStart: (source) => window.connectionStarts.push(source) };
      flow.renderFlow(snapshot, handlers);
    };
    await page.evaluate(initialize, { fixture });
    await page.locator(".flow-frontplate").first().waitFor();
    const sourceHandle = page.locator('.faceplate-handle[data-port-id="out-0"]');
    await sourceHandle.hover();
    const handleBox = await sourceHandle.boundingBox();
    await page.mouse.down();
    await page.mouse.move(handleBox.x + handleBox.width / 2 + 20, handleBox.y + handleBox.height / 2 + 20);
    await page.mouse.up();
    assert.deepEqual(await page.evaluate(() => connectionStarts), ["out-0"]);
    await page.evaluate(() => {
      snapshot.authoring.connection_candidates = [
        { sink_identity: "port/in-1", compatible: false, diagnostic: "Exact canonical refusal", adapters: [] },
        { sink_identity: "port/in-2", compatible: true, diagnostic: null, adapters: [] },
      ];
      window.before = JSON.stringify(snapshot);
      flow.renderFlow(snapshot, handlers);
    });
    await page.locator('.faceplate-port[data-port-id="in-1"][data-compatibility="incompatible"]').waitFor();
    assert.equal(await page.locator('.faceplate-port[data-port-id="in-1"]').getAttribute("title"), "Exact canonical refusal");
    assert.equal(await page.locator('.faceplate-port[data-port-id="in-2"]').getAttribute("data-compatibility"), "compatible");
    await page.evaluate(async (fixture) => { await flow.importFlowWorkspace(JSON.stringify(fixture)); }, fixture);
    await page.locator(".workspace-frame").waitFor({ timeout: 5000 }).catch(async (error) => {
      throw new Error(`${error.message}; page errors: ${JSON.stringify(errors)}; rendered: ${await page.locator("#flow-root").innerHTML()}`);
    });
    assert.equal(await page.locator(".workspace-note").textContent(), "A bend is not a runtime hop.");
    assert.equal(await page.locator(".react-flow__edge").count(), 4);
    const first = await page.evaluate(() => flow.flowSceneSnapshot().nodes.map((node) => node.position));
    await page.evaluate(async () => {
      await flow.selectFlowLayout("Vertical");
      await flow.saveFlowLayout("Personal");
      await flow.annotateFlow({ id: "personal", text: "Only presentation", subject: "gear/source", x: 20, y: 30 });
      await flow.routeFlow("cord/fourth", "manual", [{ x: 400, y: 300 }, { x: 600, y: 500 }]);
    });
    await page.getByText("Only presentation", { exact: true }).waitFor();
    const result = await page.evaluate(async () => {
      await flow.flowStorageSettled();
      const encoded = flow.exportFlowWorkspace();
      const key = [...store.keys()].find((key) => key.startsWith("conduit.patchbay.flow/") && !key.endsWith("/workspaces"));
      const changed = JSON.parse(encoded); changed.basis.checked_plot_id = "checked/changed";
      let refusal;
      try { await flow.importFlowWorkspace(JSON.stringify(changed)); } catch (error) { refusal = error.code; }
      return { encoded, stored: store.get(key), status: flow.flowWorkspaceStatus(), unchanged: before === JSON.stringify(snapshot),
        positions: flow.flowSceneSnapshot().nodes.map((node) => node.position), validations, refusal };
    });
    assert.equal(result.unchanged, true);
    assert.notDeepEqual(first, result.positions);
    assert.equal(result.status.active_layout, "Personal");
    assert.equal(result.refusal, "ChangedBasis");
    assert.deepEqual(JSON.parse(result.stored), JSON.parse(result.encoded));
    assert.ok(result.validations >= 5);
    assert.equal(result.encoded.includes("wrapper-"), false);
    const stored = await page.evaluate(() => [...store.entries()]);
    await page.reload();
    await page.evaluate(initialize, { fixture, stored });
    await page.waitForFunction(() => window.flow?.flowWorkspaceStatus().active_layout === "Personal");
    await page.getByText("Only presentation", { exact: true }).waitFor();
    assert.deepEqual(await page.evaluate(() => flow.flowSceneSnapshot().nodes.map((node) => node.position)), result.positions);
    const future = JSON.parse(result.stored); future.schema = "conduit.patchbay.workspace/v99";
    const futureEncoded = JSON.stringify(future);
    const futureStored = stored.map(([key, value]) => [key, typeof value === "string" ? futureEncoded : value]);
    await page.reload();
    await page.evaluate(initialize, { fixture, stored: futureStored });
    await page.waitForFunction(() => window.flow?.flowWorkspaceStatus().status === "UnsupportedSchema");
    assert.equal(await page.evaluate(() => [...store.values()].find((value) => typeof value === "string")), futureEncoded);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
