import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";

test("retired Tour and Creche products have no source trees", () => {
  assert.deepEqual(readdirSync("targets/browser/host/assets").filter((name) => /^(book|tour|creche)[.-]/.test(name)), []);
  assert.equal(existsSync("products/creche"), false);
  assert.ok(existsSync("targets/browser/patchbay-workbench/assets/patchbay.application.template.json"));
});

test("browser WebRTC realization is owned by the browser Host", () => {
  const names = [
    "body-webrtc-session.mjs",
    "body-webrtc-sessions.mjs",
    "webrtc-datachannel-line.mjs",
    "webrtc-session-runtime.mjs",
  ];
  for (const name of names) {
    assert.ok(existsSync(`targets/browser/host/assets/${name}`), `browser Host is missing ${name}`);
    assert.ok(!existsSync(`targets/browser/patchbay-workbench/assets/${name}`), `Patchbay still owns ${name}`);
  }
  const descriptor = JSON.parse(
    readFileSync("targets/browser/patchbay-workbench/assets/patchbay.application.template.json", "utf8"),
  );
  for (const role of ["body-webrtc-sessions", "body-webrtc-session", "webrtc-line", "webrtc-runtime"]) {
    const resource = descriptor.resources.find((candidate) => candidate.role === role);
    assert.match(resource.source, /^targets\/browser\/host\/assets\//);
  }
});

test("generic browser membership is owned by the browser Host", () => {
  const source = readFileSync("targets/browser/host/assets/browser-membership.js", "utf8");
  assert.match(source, /export async function joinBrowserBody/);
  assert.match(source, /\.\/body-webrtc-sessions\.mjs/);
  assert.doesNotMatch(source, /Patchbay/);
  assert.equal(existsSync("targets/browser/patchbay-workbench/assets/browser-membership.js"), false);
});

test("Workspace package dependencies name real source owners", () => {
    const root = resolve("targets/browser/workspace");
    const descriptor = JSON.parse(readFileSync(`${root}/workspace.application.template.json`, "utf8"));
    const resources = new Map(descriptor.resources.map((resource) => [resource.role, resource]));
    assert.equal(descriptor.application_id, "conduit.application/workspace");
    for (const resource of descriptor.resources) {
      const source = resource.source ? resolve(resource.source) : resolve(root, resource.path);
      if (!existsSync(source) || resource.kind !== "module") continue;
      for (const dependency of resource.dependencies) {
        const dependencyResource = resources.get(dependency.role);
        const dependencySource = dependencyResource?.source
          ? resolve(dependencyResource.source)
          : resolve(root, dependency.specifier);
        assert.ok(existsSync(dependencySource), `${resource.role}: missing source owner ${dependency.specifier}`);
      }
    }
});

test("target source moves preserve declared browser resource URLs and relative dependencies", () => {
  const root = resolve("targets/browser/workspace");
  const descriptor = JSON.parse(readFileSync(`${root}/workspace.application.template.json`, "utf8"));
  const resources = new Map(descriptor.resources.map((resource) => [resource.role, resource]));
  const entry = descriptor.resources.find((resource) => resource.path === "creche-installed-targets.mjs");
  const packageRoot = new URL("https://conduit.invalid/workspace/");
  const adapters = [];
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
    adapters.push(dependency.role.replace(/-adapter$/, ""));
  }
  assert.deepEqual(adapters, [
    "avr",
    "rp2040",
    "esp32",
    "std",
    "browser",
    "orange-pi",
    "raspberry-pi",
    "conduitos",
  ]);
});
