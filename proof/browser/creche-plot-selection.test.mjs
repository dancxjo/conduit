import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  encodedPlotSelection,
  openPlotSelection,
  persistedPlotSelection,
  searchPlots,
  setPlotSelected,
  togglePlot,
} from "../../targets/browser/workspace/reviewed-plot-selection.mjs";
import { initialPlotSelectionNotice, selectedCanonicalSource } from "../../targets/browser/workspace/body-bootstrap.mjs";

const inventory = Object.freeze({
  schema: "conduit.creche/reviewed-plot-inventory@2",
  source_document_id: "source/current",
  maximum_selection: 3,
  plots: [
    { name: "clock", title: "Clock", source: "plot clock {}\n", source_document_id: "source/clock", checked_plot_id: "checked/clock", required_kinds: ["time/every"] },
    { name: "desk_telegraph", title: "Desk Telegraph", source: "plot desk_telegraph {}\n", source_document_id: "source/telegraph", checked_plot_id: "checked/telegraph", required_kinds: ["presentation/text", "text/literal"] },
    { name: "memory_lantern", title: "Memory Lantern", source: "plot memory_lantern {}\n", source_document_id: "source/lantern", checked_plot_id: "checked/lantern", required_kinds: ["presentation/text"] },
  ],
});

test("Body Workspace owns bootstrap without Crèche source facades", async () => {
  const workspace = await readFile(new URL("../../targets/browser/workspace/workspace.mjs", import.meta.url), "utf8");
  const descriptor = JSON.parse(await readFile(new URL("../../targets/browser/workspace/workspace.application.template.json", import.meta.url), "utf8"));
  assert.match(workspace, /\.\/body-bootstrap\.mjs/);
  assert.match(workspace, /\.\/reviewed-plot-selection\.mjs/);
  assert.doesNotMatch(workspace, /products\/creche\/(?:browser\/)?(?:creche\.mjs|creche-lifecycle\.mjs|creche-plot-selection\.mjs)|\.\.\/\.\.\/creche\/browser\/(?:creche\.mjs|creche-lifecycle\.mjs|creche-plot-selection\.mjs)/);
  assert.equal(descriptor.resources.find(({ role }) => role === "creche-lifecycle").path, "body-bootstrap.mjs");
  assert.equal(descriptor.resources.find(({ role }) => role === "creche-plot-selection").path, "reviewed-plot-selection.mjs");
});

test("native checkbox values set selection idempotently", () => {
  const once = setPlotSelected(inventory, [], "clock", true);
  assert.strictEqual(setPlotSelected(inventory, once, "clock", true), once);
  assert.deepEqual(setPlotSelected(inventory, once, "clock", false), []);
  assert.strictEqual(setPlotSelected(inventory, [], "clock", false).length, 0);
  assert.throws(() => setPlotSelected(inventory, [], "clock", "true"), /boolean/);
});

test("browse, search, add, remove, and exact encoding share one bounded inventory", () => {
  assert.deepEqual(searchPlots(inventory, "text").map(({ name }) => name), ["desk_telegraph", "memory_lantern"]);
  let selected = togglePlot(inventory, [], "clock");
  selected = togglePlot(inventory, selected, "desk_telegraph");
  assert.deepEqual(JSON.parse(encodedPlotSelection(selected)), [
    { name: "clock", source_document_id: "source/clock", checked_plot_id: "checked/clock" },
    { name: "desk_telegraph", source_document_id: "source/telegraph", checked_plot_id: "checked/telegraph" },
  ]);
  selected = togglePlot(inventory, selected, "clock");
  assert.deepEqual(selected.map(({ name }) => name), ["desk_telegraph"]);
  assert.equal(
    selectedCanonicalSource([inventory.plots[0], inventory.plots[1]]),
    "plot clock {}\n\nform desk_telegraph {}",
  );
  assert.equal(selectedCanonicalSource([]), "");
});

test("restoration and Gallery handoff revalidate exact identities without privilege", () => {
  const retained = persistedPlotSelection(inventory, [inventory.plots[0]]);
  retained.plots.push({ name: "memory_lantern", source_document_id: "source/old", checked_plot_id: "checked/old" });
  const opened = openPlotSelection(inventory, retained, inventory.plots[1]);
  assert.deepEqual(opened.selected.map(({ name }) => name), ["clock", "desk_telegraph"]);
  assert.equal(opened.refusals[0].disposition, "stale-plot-identity");
  assert.equal(opened.refusals[0].origin, "restored");
  assert.strictEqual(opened.acceptedHandoff, inventory.plots[1]);
  assert.match(initialPlotSelectionNotice(opened), /Desk Telegraph was revalidated and preselected from Gallery/);
  assert.match(initialPlotSelectionNotice(opened), /no Body has been born/);

  const staleHandoff = openPlotSelection(inventory, null, {
    name: "desk_telegraph",
    source_document_id: "source/stale",
    checked_plot_id: "checked/telegraph",
  });
  assert.equal(staleHandoff.acceptedHandoff, null);
  assert.equal(staleHandoff.refusals[0].origin, "gallery-handoff");
  assert.equal(
    initialPlotSelectionNotice(staleHandoff),
    "The Gallery Plot handoff was stale or substituted and was not selected.",
  );
});

test("invalid, duplicate, and over-capacity state is explicit", () => {
  const duplicateInventory = { ...inventory, plots: [...inventory.plots, inventory.plots[0]], maximum_selection: 4 };
  assert.throws(() => openPlotSelection(duplicateInventory), /duplicate/);
  assert.throws(() => openPlotSelection(inventory, { schema: "wrong", plots: [] }), /malformed/);
  assert.throws(() => togglePlot({ ...inventory, maximum_selection: 0, plots: [] }, [], "clock"), /absent/);
  assert.throws(() => searchPlots(inventory, "x".repeat(129)), /bound/);
});
