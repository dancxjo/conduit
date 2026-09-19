#!/usr/bin/env node

import { cp, lstat, mkdir, readFile, readdir, rm } from "node:fs/promises";
import path from "node:path";

const GALLERY_SCHEMA = "conduit.visual-evidence-gallery/v1";
const MAXIMUM_FILES = 128;
const MAXIMUM_FILE_BYTES = 64 * 1024 * 1024;
const MAXIMUM_TOTAL_BYTES = 192 * 1024 * 1024;
const ROOT_ENTRIES = new Set([".nojekyll", "index.html", "gallery.json", "current", "commits"]);
const JOURNEYS = new Map([
  ["one-form-two-fronts", ["index.html", "manifest.json", "native.png", "native.json", "browser.png", "browser.json"]],
  ["little-life", ["index.html", "manifest.json", "t000.png", "t001.png", "t008.png", "t032.png", "presentation.txt", "execution.json"]],
]);
const OPTIONAL_JOURNEYS = new Map([
  ["hears-speaks", ["index.html", "manifest.json", "input.pcm", "input.wav", "recognition.json", "response.json", "output.wav", "receipt.json"]],
]);

const [galleryRoot, siteRoot, commit] = process.argv.slice(2);
if (!galleryRoot || !siteRoot || !/^[0-9a-f]{40}$/.test(commit ?? "")) {
  throw new Error("usage: stage-journey-pages-evidence.mjs GALLERY SITE 40_HEX_COMMIT");
}

const index = JSON.parse(await readFile(path.join(galleryRoot, "gallery.json"), "utf8"));
if (index.schema !== GALLERY_SCHEMA || index.current_commit !== commit
  || index.retention_commits !== 32 || JSON.stringify(index.commits) !== JSON.stringify([commit])) {
  throw new Error("sibling journey gallery is malformed, stale, or retains an unexpected commit");
}

await requireExactEntries(galleryRoot, ROOT_ENTRIES, "gallery root");
for (const [journey, files] of OPTIONAL_JOURNEYS) {
  try {
    await lstat(path.join(galleryRoot, "current", journey));
    JOURNEYS.set(journey, files);
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
}
await requireExactEntries(path.join(galleryRoot, "current"), new Set(JOURNEYS.keys()), "current journeys");
await requireExactEntries(path.join(galleryRoot, "commits"), new Set([commit]), "commit history");
await requireExactEntries(path.join(galleryRoot, "commits", commit), new Set(JOURNEYS.keys()), "commit journeys");
for (const [journey, files] of JOURNEYS) {
  const expected = new Set(files);
  await requireExactEntries(path.join(galleryRoot, "current", journey), expected, `current ${journey}`);
  await requireExactEntries(path.join(galleryRoot, "commits", commit, journey), expected, `commit ${journey}`);
}
await enforceBounds(galleryRoot);

const destination = path.join(siteRoot, "journeys");
await rm(destination, { recursive: true, force: true });
await mkdir(path.dirname(destination), { recursive: true });
await cp(galleryRoot, destination, { recursive: true, errorOnExist: true });
await requireAuthoredEntrance(siteRoot);
console.log(`STAGED sibling journey gallery for ${commit}`);

async function requireExactEntries(directory, expected, label) {
  const entries = await readdir(directory, { withFileTypes: true });
  const names = entries.map((entry) => entry.name).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(names) !== JSON.stringify(wanted)) {
    throw new Error(`${label} inventory differs: ${JSON.stringify(names)}`);
  }
}

async function enforceBounds(root) {
  let files = 0;
  let bytes = 0;
  async function visit(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const child = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) throw new Error(`journey gallery refuses symlink ${child}`);
      if (entry.isDirectory()) {
        await visit(child);
      } else if (entry.isFile()) {
        const metadata = await lstat(child);
        if (metadata.size > MAXIMUM_FILE_BYTES) {
          throw new Error(`journey gallery file exceeds its bound: ${child}`);
        }
        files += 1;
        bytes += metadata.size;
      } else {
        throw new Error(`journey gallery refuses special entry ${child}`);
      }
    }
  }
  await visit(root);
  if (files === 0 || files > MAXIMUM_FILES || bytes > MAXIMUM_TOTAL_BYTES) {
    throw new Error(`journey gallery tree violates its finite bounds: ${files} files, ${bytes} bytes`);
  }
}

async function requireAuthoredEntrance(root) {
  const indexPath = path.join(root, "index.html");
  const html = await readFile(indexPath, "utf8");
  if (!/href=["'][^"']*journeys\/["']/.test(html)) {
    throw new Error("Pages root does not contain its authored Journeys entrance");
  }
}
