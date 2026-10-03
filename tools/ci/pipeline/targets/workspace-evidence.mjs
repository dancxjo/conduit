import { copyFileSync, mkdirSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

// The target receipt binds these original captures to the tested source.
export function retainWorkspaceEvidence(results, destination, actions) {
  const reports = readdirSync(results, { recursive: true })
    .filter(name => path.basename(name) === 'workspace-proof.json');
  if (reports.length !== 1) throw new Error('Expected one current Workspace proof');
  const source = path.dirname(path.join(results, reports[0]));
  const proof = JSON.parse(readFileSync(path.join(source, 'workspace-proof.json'), 'utf8'));
  if (proof.schema !== 'conduit.browser/workspace-journey-proof@1'
      || JSON.stringify(proof.observations?.map(step => step.action)) !== JSON.stringify(actions.map(step => step.id))) {
    throw new Error('Workspace proof does not cover the shared action sequence');
  }
  mkdirSync(destination);
  for (const name of ['workspace-proof.json', ...actions.map(step => `${step.capture}.png`)]) {
    if (!/^[a-z0-9.-]+$/.test(name)) throw new Error('Invalid Workspace capture path');
    copyFileSync(path.join(source, name), path.join(destination, name));
  }
}
