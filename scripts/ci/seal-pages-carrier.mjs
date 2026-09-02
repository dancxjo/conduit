#!/usr/bin/env node

import { createHash } from "node:crypto";
import { cp, lstat, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const SCHEMA = "conduit.pages/carrier@1";
const MAXIMUM_FILES = 4096;
const MAXIMUM_FILE_BYTES = 256 * 1024 * 1024;
const MAXIMUM_TOTAL_BYTES = 1024 * 1024 * 1024;

const [siteSource, carrierRoot, sourceCommit, sourceTree] = process.argv.slice(2);
if (!siteSource || !carrierRoot || !isIdentity(sourceCommit) || !isIdentity(sourceTree)) {
  throw new Error("usage: seal-pages-carrier.mjs SITE CARRIER 40_HEX_COMMIT 40_HEX_TREE");
}

await rm(carrierRoot, { recursive: true, force: true });
await mkdir(carrierRoot, { recursive: true });
await cp(siteSource, path.join(carrierRoot, "site"), { recursive: true, errorOnExist: true });

const files = await inventory(path.join(carrierRoot, "site"));
const manifest = {
  schema: SCHEMA,
  source_commit: sourceCommit,
  source_tree: sourceTree,
  bounds: {
    maximum_files: MAXIMUM_FILES,
    maximum_file_bytes: MAXIMUM_FILE_BYTES,
    maximum_total_bytes: MAXIMUM_TOTAL_BYTES,
  },
  content_digest: contentDigest(files),
  files,
};
await writeFile(path.join(carrierRoot, "provenance.json"), `${JSON.stringify(manifest, null, 2)}\n`);

async function inventory(root) {
  const files = [];
  await visit(root, "", files);
  files.sort((left, right) => left.path.localeCompare(right.path));
  if (files.length === 0 || files.length > MAXIMUM_FILES) {
    throw new Error(`Pages carrier file count ${files.length} violates its finite bound`);
  }
  const total = files.reduce((sum, file) => sum + file.bytes, 0);
  if (total > MAXIMUM_TOTAL_BYTES) {
    throw new Error(`Pages carrier byte count ${total} violates its finite bound`);
  }
  return files;
}

async function visit(root, relative, files) {
  const directory = path.join(root, relative);
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const child = path.join(relative, entry.name);
    const portable = child.split(path.sep).join("/");
    if (entry.isSymbolicLink()) throw new Error(`Pages carrier refuses symlink ${portable}`);
    if (entry.isDirectory()) {
      await visit(root, child, files);
      continue;
    }
    if (!entry.isFile()) throw new Error(`Pages carrier refuses non-file ${portable}`);
    const absolute = path.join(root, child);
    const metadata = await lstat(absolute);
    if (metadata.size > MAXIMUM_FILE_BYTES) {
      throw new Error(`Pages carrier file ${portable} violates its finite byte bound`);
    }
    const bytes = await readFile(absolute);
    files.push({ path: portable, bytes: metadata.size, sha256: sha256(bytes) });
  }
}

function contentDigest(files) {
  const hash = createHash("sha256");
  hash.update(`${SCHEMA}\0`);
  for (const file of files) hash.update(`${file.path}\0${file.bytes}\0${file.sha256}\n`);
  return `sha256:${hash.digest("hex")}`;
}

function sha256(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function isIdentity(value) {
  return typeof value === "string" && /^[0-9a-f]{40}$/.test(value);
}
