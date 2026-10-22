import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

test("Workspace staging copies all declared canonical Patchbay presentation resources", async () => {
  const script = await readFile("targets/browser/tools/stage-browser-workspace.sh", "utf8");
  const template = JSON.parse(await readFile("targets/browser/workspace/workspace.application.template.json", "utf8"));
  const roles = ["browser-sdk-patchbay-play", "browser-sdk-face-actions", "browser-sdk-patchbay", "owner-face-component", "owner-face-look"];
  const resources = roles.map(role => {
    const resource = template.resources.find(value => value.role === role);
    assert.ok(resource, `${role}: declared resource`);
    return resource;
  });
  // Execute the actual copy statements, without running release catalog or Plot
  // bundling commands. This proves these staged bytes and dependency admission,
  // not a complete release or browser execution.
  const start = script.indexOf("for asset in browser-sdk-patchbay-play.mjs ");
  const end = script.indexOf('\nfor asset in creche-rendezvous.mjs ', start);
  assert.ok(start >= 0 && end > start, "explicit trusted presentation copy block");
  const scratch = await mkdtemp(path.join(tmpdir(), "conduit-workspace-patchbay-stage-"));
  try {
    const copied = spawnSync("sh", ["-eu", "-c", script.slice(start, end)], {
      env: { ...process.env, destination: scratch }, encoding: "utf8",
    });
    assert.equal(copied.status, 0, copied.stderr);
    for (const resource of resources) {
      const staged = await readFile(path.join(scratch, resource.path));
      const source = await readFile(path.resolve("targets/browser/workspace", resource.source));
      assert.deepEqual(staged, source, `${resource.role}: exact source bytes`);
    }
    const input = path.join(scratch, "template.json");
    await writeFile(input, JSON.stringify({ ...template, resources }));
    const built = spawnSync(process.execPath, ["targets/browser/tools/build-browser-application-package.mjs", input, scratch, "patchbay.application.json"], { encoding: "utf8" });
    assert.equal(built.status, 0, built.stderr);
    const manifest = JSON.parse(await readFile(path.join(scratch, "patchbay.application.json"), "utf8"));
    for (const resource of manifest.resources) {
      const bytes = await readFile(path.join(scratch, resource.path));
      assert.equal(resource.sha256, `sha256:${createHash("sha256").update(bytes).digest("hex")}`);
    }
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});
