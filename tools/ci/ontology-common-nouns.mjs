import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const ontologyNouns =
  "Body|Bodies|Host|Hosts|Form|Forms|Kind|Kinds|Gear|Gears|Front|Fronts|Face|Faces|Plan|Plans|Play|Plays|Flow|Flows|Evidence|Seed|Seeds|Birth|Wake|Wakes|Lull";
const determiners =
  "A|An|The|This|That|Another|One|Each|Every|Same|Current|Exact|Portable|Remote|Local|Ordinary|Selected|Authored|Checked|Planned|Running|New|Durable|Installed|Available|Admitted|Active|Retained|Reviewed|Shared|First|Second|Zero|Single|Its|Their|Our|Your|a|an|the|this|that|another|one|each|every|same|current|exact|portable|remote|local|ordinary|selected|authored|checked|planned|running|new|durable|installed|available|admitted|active|retained|reviewed|shared|first|second|zero|single|its|their|our|your";

const suspicious = new RegExp(`\\b(?:${determiners}) (?:${ontologyNouns})\\b`, "g");
const extensions = /\.(?:conduit|html|js|json|md|mjs|rs|sh|svg|toml|txt|ya?ml)$/;
const excluded = [
  ".github/workflows/",
  "docs/history/",
  "products/patchbay/native/assets/",
  "tools/ci/ontology-common-nouns.mjs",
];

export function ontologyCasingViolations(root = process.cwd()) {
  const files = execFileSync("git", ["ls-files", "-z"], {
    cwd: root,
    encoding: "utf8",
  })
    .split("\0")
    .filter(Boolean)
    .filter((path) => extensions.test(path))
    .filter((path) => !excluded.some((prefix) => path.startsWith(prefix)));
  const violations = [];
  for (const path of files) {
    const lines = readFileSync(`${root}/${path}`, "utf8").split("\n");
    for (const [index, line] of lines.entries()) {
      const trimmed = line.trimStart();
      if (path.endsWith(".rs") && !trimmed.startsWith("//") && !line.includes('"')) {
        continue;
      }
      suspicious.lastIndex = 0;
      if (suspicious.test(line.replaceAll(/`[^`]*`/g, ""))) {
        violations.push(`${path}:${index + 1}:${line.trim()}`);
      }
    }
  }
  return violations;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const violations = ontologyCasingViolations();
  if (violations.length > 0) {
    console.error(
      "Ontology terms are common nouns unless independently proper names:\n" +
        violations.join("\n"),
    );
    process.exitCode = 1;
  }
}
