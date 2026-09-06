#!/usr/bin/env node

import { createHash } from "node:crypto";
import { lstat, readdir, readFile } from "node:fs/promises";
import path from "node:path";

const SCHEMA = "conduit.pages/carrier@1";
const [carrierRoot, expectedTree] = process.argv.slice(2);
if (!carrierRoot || !/^[0-9a-f]{40}$/.test(expectedTree ?? "")) {
  throw new Error("usage: verify-pages-carrier.mjs CARRIER EXPECTED_40_HEX_TREE");
}

const manifest = JSON.parse(await readFile(path.join(carrierRoot, "provenance.json"), "utf8"));
if (manifest.schema !== SCHEMA || manifest.source_tree !== expectedTree
  || !/^[0-9a-f]{40}$/.test(manifest.source_commit ?? "")
  || !manifest.bounds || !Array.isArray(manifest.files)) {
  throw new Error("Pages carrier provenance identity is absent, malformed, or stale");
}

const files = [];
await visit(path.join(carrierRoot, "site"), "", files, manifest.bounds.maximum_file_bytes);
files.sort((left, right) => left.path.localeCompare(right.path));
const total = files.reduce((sum, file) => sum + file.bytes, 0);
if (files.length === 0 || files.length > manifest.bounds.maximum_files
  || total > manifest.bounds.maximum_total_bytes
  || JSON.stringify(files) !== JSON.stringify(manifest.files)
  || contentDigest(files) !== manifest.content_digest) {
  throw new Error("Pages carrier content differs from its admitted finite provenance");
}
console.log(`VERIFIED ${manifest.content_digest} for tree ${expectedTree}`);

async function visit(root, relative, files, maximumFileBytes) {
  for (const entry of await readdir(path.join(root, relative), { withFileTypes: true })) {
    const child = path.join(relative, entry.name);
    const portable = child.split(path.sep).join("/");
    if (entry.isSymbolicLink()) throw new Error(`Pages carrier refuses symlink ${portable}`);
    if (entry.isDirectory()) {
      await visit(root, child, files, maximumFileBytes);
      continue;
    }
    if (!entry.isFile()) throw new Error(`Pages carrier refuses non-file ${portable}`);
    const absolute = path.join(root, child);
    const metadata = await lstat(absolute);
    if (metadata.size > maximumFileBytes) {
      throw new Error(`Pages carrier file ${portable} violates its finite byte bound`);
    }
    files.push({
      path: portable,
      bytes: metadata.size,
      sha256: `sha256:${createHash("sha256").update(await readFile(absolute)).digest("hex")}`,
    });
  }
}

function contentDigest(files) {
  const hash = createHash("sha256");
  hash.update(`${SCHEMA}\0`);
  for (const file of files) hash.update(`${file.path}\0${file.bytes}\0${file.sha256}\n`);
  return `sha256:${hash.digest("hex")}`;
}
