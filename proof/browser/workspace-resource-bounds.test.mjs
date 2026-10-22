import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

async function workspaceResourceBytes(resource) {
  const candidates = [
    resource.path.replace("/browser-deployment/", "/deployment/browser/"),
    path.join("targets/browser/workspace", resource.path),
    path.join("targets/browser/host/assets", resource.path),
    path.join("targets/browser/sdk", resource.path),
    path.join("targets/browser/handbook", resource.path),
  ];
  let bytes;
  for (const candidate of candidates) {
    try { bytes = await readFile(candidate); break; }
    catch (error) { if (error.code !== "ENOENT") throw error; }
  }
  assert.ok(bytes, `${resource.role}: shipped source must be located`);
  return bytes;
}

test("every shipped Workspace module fits its declared finite package bound", async () => {
  const template = JSON.parse(await readFile("targets/browser/workspace/workspace.application.template.json", "utf8"));
  for (const resource of template.resources.filter(value => value.kind === "module")) {
    const bytes = await workspaceResourceBytes(resource);
    assert.ok(bytes, `${resource.role}: shipped source must be located`);
    assert.ok(bytes.length > 0 && bytes.length <= resource.maximum_bytes,
      `${resource.role}: ${bytes.length} module bytes exceed declared ${resource.maximum_bytes}`);
  }
});

test("Workspace admits its current storage module and still refuses an undersized declaration", async () => {
  const template = JSON.parse(await readFile("targets/browser/workspace/workspace.application.template.json", "utf8"));
  const resource = template.resources.find(value => value.role === "browser-application-storage");
  const bytes = await readFile("targets/browser/host/assets/browser-application-storage.mjs");
  assert.ok(bytes.length > 0 && bytes.length <= resource.maximum_bytes);
  assert.deepEqual(resource.dependencies, []);
  const scratch = await mkdtemp(path.join(tmpdir(), "conduit-workspace-resource-"));
  try {
    await writeFile(path.join(scratch, resource.path), bytes);
    // This is the real resource and producer's admission boundary, not proof
    // that a complete application was staged or executed in a browser.
    const input = path.join(scratch, "template.json");
    const producer = path.resolve("targets/browser/tools/build-browser-application-package.mjs");
    await writeFile(input, JSON.stringify({ ...template, resources: [resource] }));
    const built = spawnSync(process.execPath, [producer, input, scratch, "storage.application.json"], { encoding: "utf8" });
    assert.equal(built.status, 0, built.stderr);
    const manifest = JSON.parse(await readFile(path.join(scratch, "storage.application.json"), "utf8"));
    assert.equal(manifest.resources[0].maximum_bytes, resource.maximum_bytes);
    assert.equal(manifest.resources[0].sha256, `sha256:${createHash("sha256").update(bytes).digest("hex")}`);
    await writeFile(input, JSON.stringify({ ...template, resources: [{ ...resource, maximum_bytes: bytes.length - 1 }] }));
    const refused = spawnSync(process.execPath, [producer, input, scratch, "refused.application.json"], { encoding: "utf8" });
    assert.notEqual(refused.status, 0);
    assert.match(refused.stderr, /application resource exceeds bound/);
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});


test("Workspace packages the Body host and owner adapter dependency closure with exact byte custody", async () => {
  const template = JSON.parse(await readFile("targets/browser/workspace/workspace.application.template.json", "utf8"));
  assert.ok(template.resources.length <= 96);
  assert.equal(template.resources.reduce((sum, resource) => sum + resource.maximum_bytes, 0), 22437888);
  assert.ok(template.resources.reduce((sum, resource) => sum + resource.maximum_bytes, 0) <= 32 * 1024 * 1024);
  const byRole = new Map(template.resources.map(resource => [resource.role, resource]));
  const included = new Set();
  function include(role) {
    if (included.has(role)) return;
    const resource = byRole.get(role);
    assert.ok(resource, `declared dependency ${role} must exist`);
    assert.equal(resource.kind, "module");
    included.add(role);
    for (const dependency of resource.dependencies) include(dependency.role);
  }
  include("browser-body-host");
  include("workspace-play");
  const resources = template.resources.filter(resource => included.has(resource.role));
  const bodyHost = byRole.get("browser-body-host");
  assert.equal(bodyHost.maximum_bytes, 48 * 1024);
  const scratch = await mkdtemp(path.join(tmpdir(), "conduit-workspace-body-host-"));
  const originals = new Map();
  try {
    for (const resource of resources) {
      const bytes = await workspaceResourceBytes(resource);
      assert.ok(bytes.length > 0 && bytes.length <= resource.maximum_bytes);
      originals.set(resource.role, bytes);
      const destination = path.join(scratch, resource.path);
      await mkdir(path.dirname(destination), { recursive: true });
      await writeFile(destination, bytes);
    }
    const bytes = originals.get("browser-body-host");
    assert.ok(bytes.length > 32 * 1024 && bytes.length <= bodyHost.maximum_bytes,
      `actual Body host module must cross 32 KiB within its role bound: ${bytes.length}`);
    const input = path.join(scratch, "template.json");
    const producer = path.resolve("targets/browser/tools/build-browser-application-package.mjs");
    await writeFile(input, JSON.stringify({ ...template, resources }));
    const built = spawnSync(process.execPath, [producer, input, scratch, "body-host.application.json"], { encoding: "utf8" });
    assert.equal(built.status, 0, built.stderr);
    const manifest = JSON.parse(await readFile(path.join(scratch, "body-host.application.json"), "utf8"));
    assert.equal(manifest.resources.length, resources.length);
    for (const resource of manifest.resources) {
      assert.equal(resource.sha256, `sha256:${createHash("sha256").update(originals.get(resource.role)).digest("hex")}`);
      assert.deepEqual(resource.dependencies, byRole.get(resource.role).dependencies);
      assert.equal(resource.maximum_bytes, byRole.get(resource.role).maximum_bytes);
    }
    for (const maximum of [32768, bytes.length - 1]) {
      await writeFile(input, JSON.stringify({ ...template, resources: resources.map(resource =>
        resource.role === "browser-body-host" ? { ...resource, maximum_bytes: maximum } : resource) }));
      const refused = spawnSync(process.execPath, [producer, input, scratch, "refused.application.json"], { encoding: "utf8" });
      assert.notEqual(refused.status, 0);
      assert.match(refused.stderr, /application resource exceeds bound: browser-body-host/);
      await assert.rejects(readFile(path.join(scratch, "refused.application.json")), { code: "ENOENT" });
    }
    // An omitted import declaration must remain a refusal, not a packaging shortcut.
    await writeFile(input, JSON.stringify({ ...template, resources: resources.map(resource =>
      resource.role === "browser-body-host" ? { ...resource, dependencies: resource.dependencies.slice(1) } : resource) }));
    const undeclared = spawnSync(process.execPath, [producer, input, scratch, "undeclared.application.json"], { encoding: "utf8" });
    assert.notEqual(undeclared.status, 0);
    assert.match(undeclared.stderr, /imports undeclared module/);
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});
