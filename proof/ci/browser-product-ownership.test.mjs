import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

test("browser Host has no Tour or Creche product source", () => {
  assert.deepEqual(readdirSync("targets/browser/host/assets").filter((name) => /^(book|tour|creche)[.-]/.test(name)), []);
  assert.ok(existsSync("products/patchbay/html/assets/patchbay.application.template.json"));
});

for (const product of ["tour", "creche"]) {
  test(`${product} package dependencies name real source owners`, () => {
    const root = resolve(`products/${product}/browser`);
    const descriptor = JSON.parse(readFileSync(`${root}/${product}.application.template.json`, "utf8"));
    assert.equal(descriptor.application_id, `conduit.application/${product}`);
    for (const resource of descriptor.resources) {
      if (!existsSync(resolve(root, resource.path)) || resource.kind !== "module") continue;
      for (const dependency of resource.dependencies) {
        assert.ok(existsSync(resolve(root, dependency.specifier)), `${resource.role}: missing source owner ${dependency.specifier}`);
      }
    }
  });
}
