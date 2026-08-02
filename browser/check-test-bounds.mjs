import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";

const roots = ["browser", "tour"];
const forbidden = [
  [/(?:test|testInfo)\.setTimeout\s*\(/, "per-test timeout override"],
  [/\btest\.slow\s*\(/, "slow-test timeout multiplier"],
];
const violations = [];

for (const root of roots) {
  for (const name of await readdir(root)) {
    if (!name.endsWith(".spec.mjs")) continue;
    const path = join(root, name);
    const source = await readFile(path, "utf8");
    for (const [pattern, label] of forbidden) {
      if (pattern.test(source)) violations.push(`${path}: ${label}`);
    }
  }
}

if (violations.length > 0) {
  throw new Error(
    `Browser tests must stay inside their configured shard bound:\n${violations.join("\n")}`,
  );
}

console.log("Browser specs use configured shard bounds without per-test overrides.");
