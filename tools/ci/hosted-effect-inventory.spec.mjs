import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = path.resolve(import.meta.dirname, "../..");
const inventoryPath = path.join(root, "docs/architecture/hosted-effect-inventory.json");
const effectPatterns = [
  /std::fs::/,
  /File::open/,
  /OpenOptions::/,
  /TcpStream::|TcpListener::|UdpSocket::/,
  /Command::new|process::Command/,
  /tungstenite::|ureq::/,
  /\/dev\//,
];

async function rustFiles(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) result.push(...await rustFiles(file));
    else if (entry.name.endsWith(".rs")) result.push(file);
  }
  return result;
}

test("every hosted production effect path is classified", async () => {
  const inventory = JSON.parse(await readFile(inventoryPath, "utf8"));
  assert.equal(inventory.schema, "conduit.hosted-effect-inventory/v1");
  const entries = inventory.entries;
  assert.ok(entries.length > 1);
  const classified = new Set(entries.flatMap(entry => entry.source_paths));
  for (const entry of entries) {
    for (const field of ["family", "implementation_owner", "external_mechanism", "boundary", "enforcement_class", "proposed_base_owner", "migration_status", "adversarial_proof"])
      assert.ok(entry[field], `${entry.family} lacks ${field}`);
    assert.ok(entry.semantic_kinds.length > 0);
    assert.ok(entry.contracts.length > 0);
    assert.ok(entry.ambient_privileges.length > 0);
    assert.ok(
      ["cooperative", "process-isolated", "os-capability-mediated"].includes(
        entry.enforcement_class,
      ),
    );
    for (const source of entry.source_paths)
      await readFile(path.join(root, source), "utf8");
  }

  const excluded = relative => inventory.excluded_paths.some(rule =>
    (rule.path && relative === rule.path)
      || (rule.prefix && relative.startsWith(rule.prefix))
      || (rule.suffix && relative.endsWith(rule.suffix)));
  const files = await rustFiles(path.join(root, "targets/std/src"));
  const unclassified = [];
  for (const file of files) {
    const relative = path.relative(root, file).split(path.sep).join("/");
    const source = await readFile(file, "utf8");
    if (effectPatterns.some(pattern => pattern.test(source))
        && !classified.has(relative) && !excluded(relative)) unclassified.push(relative);
  }
  assert.deepEqual(unclassified, [], `unclassified ambient effect paths: ${unclassified.join(", ")}`);
});

test("isolated claims require an accepted adversarial proof", async () => {
  const inventory = JSON.parse(await readFile(inventoryPath, "utf8"));
  for (const entry of inventory.entries.filter(entry => entry.enforcement_class !== "cooperative")) {
    assert.match(entry.adversarial_proof, /^#[0-9]+$/);
    assert.match(entry.migration_status, /proof accepted/);
  }
});
