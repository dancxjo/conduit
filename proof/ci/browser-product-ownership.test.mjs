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

test("target source moves preserve declared browser resource URLs and relative dependencies", () => {
  const root = resolve("products/creche/browser");
  const descriptor = JSON.parse(readFileSync(`${root}/creche.application.template.json`, "utf8"));
  const resources = new Map(descriptor.resources.map((resource) => [resource.role, resource]));
  const entry = descriptor.resources.find((resource) => resource.path === "creche.mjs");
  const packageRoot = new URL("https://conduit.invalid/creche/");
  let adapters = 0;
  for (const dependency of entry.dependencies.filter((dependency) => dependency.role.endsWith("-adapter"))) {
    const source = resolve(root, dependency.specifier);
    assert.ok(existsSync(source), `target source owner is absent: ${dependency.specifier}`);
    const resource = resources.get(dependency.role);
    const resourceUrl = new URL(resource.path, packageRoot);
    const bytes = readFileSync(source, "utf8");
    for (const imported of resource.dependencies) {
      const target = resources.get(imported.role);
      assert.ok(target, `undeclared dependency role ${imported.role}`);
      assert.ok(bytes.includes(`"${imported.specifier}"`), `source import missing: ${imported.specifier}`);
      assert.equal(new URL(imported.specifier, resourceUrl).href, new URL(target.path, packageRoot).href);
    }
    for (const match of bytes.matchAll(/new URL\(\s*"(\.\.\/[^"\n]+)"/g)) {
      assert.ok(new URL(match[1], resourceUrl).href.startsWith(new URL("artifacts/", packageRoot).href),
        `target artifact URL escaped its package artifacts: ${match[1]}`);
    }
    adapters += 1;
  }
  assert.ok(adapters > 0, "Crèche must consume declared target adapters");
});
