import { lstatSync, readFileSync } from 'node:fs';
import path from 'node:path';

export const THREE_HOST_DEVELOPMENT_ROOT = 'site/evidence/three-host-development';
export const THREE_HOST_DEVELOPMENT_PROOF = 'journey-one-body-three-host-development';
export const THREE_HOST_DEVELOPMENT_SUITE = 'journey-gallery';
const SITE_NAVIGATION_HREFS = new Set([
  '/conduit/', '/conduit/journeys/', '/conduit/handbook/',
  '/conduit/#get-conduit', '/conduit/workspace/',
]);

export function retainedThreeHostDevelopmentEvidence(evidenceRoot = THREE_HOST_DEVELOPMENT_ROOT) {
  let root;
  try {
    root = lstatSync(evidenceRoot);
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
  if (!root.isDirectory() || root.isSymbolicLink()) {
    throw new Error('Three-host development evidence root must be a regular source directory');
  }
  const manifest = JSON.parse(readFileSync(path.join(evidenceRoot, 'manifest.json'), 'utf8'));
  const sourceCommit = manifest.git_commit;
  if (!/^[a-f0-9]{40}$/.test(sourceCommit)
    || manifest.schema !== 'conduit.evidence-manifest/v1'
    || manifest.result !== 'diagnostic-incomplete'
    || manifest.proof_id !== THREE_HOST_DEVELOPMENT_PROOF
    || manifest.suite_id !== THREE_HOST_DEVELOPMENT_SUITE) {
    throw new Error('Three-host development evidence lacks an exact-source diagnostic manifest');
  }
  const paths = new Set(manifest.outputs?.map(output => output.path));
  if (!paths.has('index.html') || !paths.has('report.json')
    || ![...paths].some(file => file.endsWith('.png'))) {
    throw new Error('Three-host development evidence lacks its page, report, or image');
  }
  const report = JSON.parse(readFileSync(path.join(evidenceRoot, 'report.json'), 'utf8'));
  const page = readFileSync(path.join(evidenceRoot, 'index.html'), 'utf8');
  if (report.native_source_commit !== sourceCommit || !report.run_id || !report.body_id
    || !/^conduit\.body\/three-host-owner-journey@1$/.test(report.schema)
    || !/Development capture · \d of 8 chapters complete/.test(page)
    || !page.includes('The public Journey and accepted release require separate gates')
    || (page.match(/<article id="(?:birth|join|start|see|hear|loss|return|lull)"/g) ?? []).length !== 8
    || (report.owner_llm_speech?.wav?.path && !paths.has(report.owner_llm_speech.wav.path))) {
    throw new Error('Three-host development page or report does not match its eight-chapter diagnostic run');
  }
  for (const [, reference] of page.matchAll(/\b(?:href|src)="([^"]+)"/g)) {
    if (!reference.startsWith('https://') && !reference.startsWith('#')
      && !SITE_NAVIGATION_HREFS.has(reference) && !paths.has(reference)) {
      throw new Error(`Three-host development page links an undeclared asset: ${reference}`);
    }
  }
  return { root: evidenceRoot, sourceCommit };
}
