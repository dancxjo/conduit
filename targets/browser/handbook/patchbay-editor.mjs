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
  let declaration = plot, sourceName = null, foundBack = false;
  while (current !== root) {
    if (visited.has(current) || visited.size >= face.subjects.length) return fail('The requested subject has cyclic or missing containment.');
    visited.add(current);
    const facts = face.properties.filter(property => property.subject === current);
    const names = facts.filter(property => property.name === 'source-plot');
    if (names.length > 1 || (names.length && typeof identity(names[0]) !== 'string')) return fail('The Source declaration name is ambiguous.');
    if (sourceName === null && names.length) sourceName = identity(names[0]);
    const backs = facts.filter(property => property.name === 'checked-back-id');
    // A composition edits its invocation; contents follow the checked Back.
    if (subject.role !== 'Cord' && current !== subject.identity && backs.length && !foundBack) {
      if (backs.length !== 1) return fail('The containing checked Back is ambiguous.');
      const matches = plots.filter(candidate => candidate.checkedPlotId === identity(backs[0]));
      if (matches.length !== 1) return fail('The containing checked Back is absent from the Source inventory.');
      declaration = matches[0]; foundBack = true;
    }
    const parents = face.relationships.filter(relation => relation.kind === 'Contains' && relation.target === current);
    if (parents.length !== 1) return fail('The requested subject has no unique Source containment.');
    const recursive = facts.filter(property => property.name === 'recursive-parent');
    if (recursive.length > 1 || (recursive.length && identity(recursive[0]) !== parents[0].source)) return fail('Recursive provenance differs from checked containment.');
    current = parents[0].source;
  }
  if (subject.role === 'Cord') {
    const facts = face.properties.filter(property => property.subject === subject.identity);
    const exact = name => {
      const values = facts.filter(property => property.name === name);
      if (values.length !== 1) return fail('The Cord has no unique declaration provenance.');
      return values[0].value;
    };
    if (exact('source-origin')?.Identity !== 'cord-stages') return fail('Generated or unproven Cord wiring cannot be edited directly.');
    const checked = exact('checked-source-plot-id')?.Identity;
    const document = exact('source-document-id')?.Identity;
    const name = exact('source-plot')?.Identity;
    const owners = plots.filter(candidate => candidate.checkedPlotId === checked
      && candidate.sourceDocumentId === document && candidate.name === name);
    if (owners.length !== 1) return fail('The Cord declaration is absent or ambiguous in the checked Source inventory.');
    declaration = owners[0];
    const start = exact('source-byte-start')?.Count, end = exact('source-byte-end')?.Count;
    const bytes = new TextEncoder().encode(declaration.source);
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end)
        || start < 0 || end <= start || end > bytes.length) return fail('The Cord Source extent is invalid.');
    let sourceRange;
    try {
      const decoder = new TextDecoder('utf-8', { fatal: true });
      sourceRange = Object.freeze({ start: decoder.decode(bytes.subarray(0, start)).length,
        end: decoder.decode(bytes.subarray(0, end)).length });
    } catch { return fail('The Cord Source extent splits a Unicode character.'); }
    return Object.freeze({ plot: declaration, subject, sourceRange });
  }
  if (sourceName !== null && sourceName !== declaration.name) return fail('The Source declaration differs from the checked owner.');
  return Object.freeze({ plot: declaration, subject });
}
