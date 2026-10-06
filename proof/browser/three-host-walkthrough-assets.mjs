// The walkthrough may later be copied without its private producer parent.
// Refuse any local link that would break or escape that self-contained bundle.
import { lstat } from 'node:fs/promises';
import path from 'node:path';

export async function verifyWalkthroughAssets(output, html) {
  const root = path.resolve(output);
  const references = [...html.matchAll(/\b(?:href|src)="([^"]+)"/g)].map(match => match[1]);
  for (const reference of references) {
    if (reference.startsWith('#') || /^https:\/\//.test(reference)) continue;
    if (/^[a-z][a-z0-9+.-]*:/i.test(reference) || reference.startsWith('/')) {
      throw new Error(`walkthrough has an unsupported asset link: ${reference}`);
    }
    const local = decodeURIComponent(reference.split(/[?#]/, 1)[0]);
    const file = path.resolve(root, local);
    if (!local || !file.startsWith(`${root}${path.sep}`)) {
      throw new Error(`walkthrough asset escapes its bundle: ${reference}`);
    }
    const info = await lstat(file).catch(() => null);
    if (!info?.isFile() || info.isSymbolicLink()) {
      throw new Error(`walkthrough asset is missing or unsafe: ${reference}`);
    }
  }
}
