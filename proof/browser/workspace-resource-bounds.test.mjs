import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

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
