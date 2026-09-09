import { browserHostOperationLimits, createBrowserHostOperations } from "../../../targets/browser/host/assets/browser-host-operations.mjs";

// A Gallery link identifies checked meaning. It neither carries executable code
// nor creates a Body; the receiver revalidates against its own admitted inventory.
export function readWorkspaceHandoff(location, inventory) {
  const parameters = new URLSearchParams(location.search);
  const keys = ['form', 'source_document_id', 'checked_form_id'];
  if (keys.every(key => !parameters.has(key))) return null;
  const values = keys.map(key => parameters.getAll(key));
  if (values.some(entries => entries.length !== 1 || !entries[0] || new TextEncoder().encode(entries[0]).length > 256)) {
    throw new Error('The Gallery link needs one complete Form identity.');
  }
  const [name, source, checked] = values.map(entries => entries[0]);
  const form = inventory.forms.find(form => form.name === name && form.source_document_id === source && form.checked_form_id === checked);
  if (!form) throw new Error('This Gallery Form is unavailable or its checked identity has changed. Your installed Forms are retained.');
  return form;
}

export async function consumeWorkspaceHandoff({ host, applicationId }) {
  const operations = createBrowserHostOperations({ hostId: host.hostId, bootId: host.bootId,
    applicationId, applicationGeneration: 1, authorityGeneration: 1 });
  const path = globalThis.location.pathname;
  const outcome = await operations.moveLocation({
    contract: browserHostOperationLimits.contract, kind: 'location', operationId: 'workspace/gallery-handoff',
    hostId: host.hostId, bootId: host.bootId, applicationId, applicationGeneration: 1,
    authorityGeneration: 1, presentationRevision: 1, mode: 'replace', path,
  });
  if (outcome.disposition !== 'completed' || outcome.path !== path) throw new Error(`Gallery handoff location refused (${outcome.disposition})`);
}
