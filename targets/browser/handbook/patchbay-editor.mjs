// Resolve checked Source identity from owner facts, never from display labels.
export function resolvePatchbayEditorTarget(observation, plots) {
  const request = observation?.pending_owner_request?.request;
  const face = observation?.face;
  const fail = message => { throw Object.assign(new Error(message), { code: 'PatchbayEditRefused' }); };
  if (request?.kind !== 'EditCurrent' || !Array.isArray(face?.properties)
      || !Array.isArray(face.subjects) || !Array.isArray(face.relationships) || !Array.isArray(plots)) {
    return fail('A current typed Edit request and checked Source inventory are required.');
  }
  const identity = property => property.value?.Identity;
  const roots = face.properties.filter(property => property.name === 'expanded-plot-id'
    && identity(property) === request.expanded_plot_id);
  if (roots.length !== 1) return fail('The requested expanded Plot has no unique Source owner.');
  const root = roots[0].subject;
  if (!face.subjects.some(subject => subject.identity === root && subject.role === 'Plot')) return fail('The requested Source Plot has no checked declaration.');
  const candidates = plots.filter(plot => `plot/${plot.checkedPlotId}` === root);
  if (candidates.length !== 1) return fail('The requested checked Plot is absent or ambiguous.');
  const plot = candidates[0];
  const documents = face.properties.filter(property => property.subject === root && property.name === 'source-document-id');
  if (documents.length !== 1 || identity(documents[0]) !== plot.sourceDocumentId) return fail('The requested Source document differs from the editor inventory.');
  const subjects = face.properties.filter(property => property.name === 'semantic-id' && identity(property) === request.subject_identity);
  if (subjects.length !== 1) return fail('The requested semantic subject is absent or ambiguous.');
  const subject = face.subjects.find(candidate => candidate.identity === subjects[0].subject);
  if (!subject) return fail('The requested subject has no checked Face declaration.');
  let current = subject.identity;
  const visited = new Set();
  while (current !== root) {
    if (visited.has(current) || visited.size >= face.subjects.length) return fail('The requested subject has cyclic or missing containment.');
    visited.add(current);
    const facts = face.properties.filter(property => property.subject === current);
    if (facts.some(property => property.name === 'recursive-parent')
        || facts.some(property => property.name === 'source-plot' && identity(property) !== plot.name)) {
      return fail('This editor cannot resolve the requested nested Source declaration.');
    }
    const parents = face.relationships.filter(relation => relation.kind === 'Contains' && relation.target === current);
    if (parents.length !== 1) return fail('The requested subject has no unique Source containment.');
    current = parents[0].source;
  }
  return Object.freeze({ plot, subject });
}
