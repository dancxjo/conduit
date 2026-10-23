import { createHash } from 'node:crypto';
import { copyFileSync, lstatSync, mkdirSync, readFileSync, readdirSync } from 'node:fs';
import { isDeepStrictEqual } from 'node:util';
import path from 'node:path';

const DIRECTORY = 'patchbay-checked-source';
// The existing collector admits 128 diagram pages and 2 * MAX_PAGES + 4 captures.
const MAX_CAPTURES = 260;
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const hash = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const refuse = message => { throw new Error(`Checked Source artifact: ${message}`); };
function regular(file) {
  if (!lstatSync(file).isFile()) refuse(`not a regular file: ${file}`);
  return readFileSync(file);
}

/** Bind the collector's separate captures to the verified journey before sealing. */
export function stageCheckedSourceEvidence(journey, evidence, destination) {
  const section = journey.patchbay_actions?.checked_source_inspection;
  // Older ordinary journeys legitimately have no checked Source inspection.
  if (section === undefined) return;
  if (section?.schema !== 'conduit.conduitos/checked-source-inspection@1'
    || !/^[0-9a-f]{40}$/.test(journey.source_commit ?? '') || !hash(journey.image_sha256)
    || typeof section.source !== 'string' || !section.source.length
    || !hash(section.source_document_id) || !hash(section.checked_plot_id) || !hash(section.expanded_plot_id)
    || !Array.isArray(section.captures) || !section.captures.length || section.captures.length > MAX_CAPTURES) {
    refuse('malformed source/image identity or finite capture set');
  }
  const source = path.join(evidence, DIRECTORY);
  if (!lstatSync(source).isDirectory()) refuse('capture directory is not a real directory');
  const manifest = regular(path.join(source, 'manifest.json'));
  if (!isDeepStrictEqual(JSON.parse(manifest), section)) refuse('manifest differs from the source-bound journey section');
  const files = new Set(['manifest.json']);
  const checkpoints = new Set();
  const retained = [];
  for (const capture of section.captures) {
    const image = capture?.image;
    const checkpoint = capture?.checkpoint;
    if (typeof checkpoint !== 'string' || !/^[a-z0-9][a-z0-9-]*$/.test(checkpoint)
      || checkpoints.has(checkpoint) || image?.checkpoint !== checkpoint
      || image.png !== `${checkpoint}.png` || files.has(image.png)
      || !Number.isSafeInteger(image.png_bytes) || image.png_bytes <= 0
      || !hash(image.png_sha256) || !hash(image.pixel_sha256)
      || image.pixel_format !== 'RGBA8'
      || !Number.isSafeInteger(image.width) || image.width <= 0
      || !Number.isSafeInteger(image.height) || image.height <= 0) {
      refuse('malformed, duplicate or escaping capture metadata');
    }
    checkpoints.add(checkpoint);
    files.add(image.png);
    const bytes = regular(path.join(source, image.png));
    if (bytes.length !== image.png_bytes || sha256(bytes) !== image.png_sha256
      || bytes.length < 33 || !bytes.subarray(0, 8).equals(Buffer.from('89504e470d0a1a0a', 'hex'))
      || bytes.readUInt32BE(8) !== 13 || bytes.toString('ascii', 12, 16) !== 'IHDR'
      || bytes.readUInt32BE(16) !== image.width || bytes.readUInt32BE(20) !== image.height
      || !bytes.subarray(24, 29).equals(Buffer.from([8, 6, 0, 0, 0]))) {
      refuse(`capture bytes differ from declared PNG: ${image.png}`);
    }
    retained.push([image.png, bytes]);
  }
  if (!isDeepStrictEqual(readdirSync(source).sort(), [...files].sort())) refuse('capture directory differs from manifest inventory');
  // Validate the complete input set before copying; never recursively follow links.
  const output = path.join(destination, DIRECTORY);
  mkdirSync(output);
  for (const [name, bytes] of [['manifest.json', manifest], ...retained]) {
    const target = path.join(output, name);
    copyFileSync(path.join(source, name), target);
    if (!regular(target).equals(bytes)) refuse(`retained copy differs: ${name}`);
  }
}
