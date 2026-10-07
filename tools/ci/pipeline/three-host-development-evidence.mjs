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
    || ![...paths].some(file => file.endsWith('.png'))
    || ![...paths].some(file => file.endsWith('.wav'))) {
    throw new Error('Three-host development evidence lacks its page, report, image, or listener audio');
  }
  const report = JSON.parse(readFileSync(path.join(evidenceRoot, 'report.json'), 'utf8'));
  const page = readFileSync(path.join(evidenceRoot, 'index.html'), 'utf8');
  if (report.native_source_commit !== sourceCommit || !report.run_id || !report.body_id
    || !report.owner_selected_speech || !report.owner_llm_speech
    || !paths.has(report.owner_llm_speech.wav?.path)
    || !page.includes('not the complete eight-chapter public journey')) {
    throw new Error('Three-host development page or report does not match its partial live run');
  }
  for (const [, reference] of page.matchAll(/\b(?:href|src)="([^"]+)"/g)) {
    if (!reference.startsWith('https://') && !reference.startsWith('#')
      && !SITE_NAVIGATION_HREFS.has(reference) && !paths.has(reference)) {
      throw new Error(`Three-host development page links an undeclared asset: ${reference}`);
    }
  }
  return { root: evidenceRoot, sourceCommit };
}
