import test from "node:test";
import assert from "node:assert/strict";
import { presentTourInventory } from "../../products/tour/browser/tour-inventory-presentation.mjs";

test("all inventory offers remain reachable inside the unchanged 40-node bound", () => {
  for (const count of [0, 32, 33, 64, 65]) {
    let current;
    const presentation = {
      present(slot, view, handlers) {
        assert.equal(slot, "tour-inventory");
        assert.ok(view.nodes.length <= 40);
        current = { view, handlers };
      },
      nextEvent(slot) { assert.equal(slot, "tour-inventory"); },
    };
    const entries = Array.from({ length: count }, (_, index) => ({
      kind_id: `kind-${index}`, implementation_id: `impl-${index}`,
      family: "math", classification: "installed", reason: "exact offer",
    }));
    presentTourInventory(presentation, { entries, limits: { maximum_gears: 16, maximum_cords: 32 } });
    const seen = [];
    let priorRevision = 0;
    for (let page = 0; page < Math.max(1, Math.ceil(count / 32)); page += 1) {
      assert.ok(current.view.revision > priorRevision);
      priorRevision = current.view.revision;
      seen.push(...current.view.nodes.filter((node) => node.component === "definition").map((node) => node.text));
      const next = current.view.nodes.find((node) => node.key === "inventory-next");
      if (next.action !== null) current.handlers.onEvent({ action: "book.inventory.next" });
    }
    assert.deepEqual(seen, entries.map((entry) => entry.kind_id));
    assert.equal(current.view.nodes.find((node) => node.key === "inventory-next").action, null);
    if (count > 32) {
      current.handlers.onEvent({ action: "book.inventory.previous" });
      assert.ok(current.view.revision > priorRevision);
    }
  }
});
