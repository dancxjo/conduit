// Borrow renderer references from the canonical Source Face, never application state.
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const refusal = message => { throw new Error(`Portable Face: ${message}`); };
export function portableFaceModel(encounter) {
  const face = encounter?.face, view = encounter?.view, bindings = encounter?.control_bindings;
  if (!face || !view || face.identity !== view.face_id || String(face.revision) !== view.face_revision
    || !Array.isArray(face.subjects) || face.subjects.length > 1024
    || !Array.isArray(face.actions) || face.actions.length > 256
    || !Array.isArray(face.properties) || face.properties.length > 4096) refusal('invalid source binding');
  if (!bindings || bindings.face_identity !== face.identity || String(bindings.face_revision) !== String(face.revision)
    || !Array.isArray(bindings.text_entries) || bindings.text_entries.length > 64
    || !Array.isArray(bindings.collections) || bindings.collections.length > 64) refusal('prepared control binding');
  if (new Set(bindings.text_entries).size !== bindings.text_entries.length
    || new Set(bindings.collections.map(binding => binding.subject)).size !== bindings.collections.length) refusal('duplicate prepared binding');
  const subjects = new Map(face.subjects.map(subject => [subject.identity, subject]));
  const actions = new Map(face.actions.map(action => [action.identity, action]));
  if (subjects.size !== face.subjects.length || actions.size !== face.actions.length) refusal('duplicate source identity');
  const property = (subject, name, optional = false) => {
    const values = face.properties.filter(item => item.subject === subject && item.name === name);
    if (values.length > 1 || (!optional && values.length !== 1)) refusal(`ambiguous or missing ${name}`);
    return values[0]?.value;
  };
  const typed = (subject, name) => {
    const value = property(subject, name)?.TypedValue;
    if (!value || !Array.isArray(value.bytes) || value.bytes.length > 8192
      || value.bytes.some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255)) refusal('invalid typed value');
    return { contract: value.contract, bytes: value.bytes, text: decoder.decode(Uint8Array.from(value.bytes)) };
  };
  const collection = subject => {
    const relationships = face.relationships.filter(relation => relation.source === subject.identity && relation.kind === 'Contains');
    if (relationships.length > 64 || property(subject.identity, 'item-count')?.Count !== relationships.length) refusal('collection capacity or membership');
    const seen = new Set(), orders = new Set();
    const items = relationships.map(relation => {
      const item = subjects.get(relation.target), order = property(relation.target, 'order')?.Count;
      if (!item || seen.has(item.identity) || !Number.isSafeInteger(order) || orders.has(order)) refusal('collection identity or order');
      seen.add(item.identity); orders.add(order);
      return { ...item, order };
    }).sort((a, b) => a.order - b.order);
    return { subject, items, mode: property(subject.identity, 'choice-multiplicity', true)?.Text ?? 'information' };
  };
  const collections = bindings.collections.map(binding => {
    const subject = subjects.get(binding.subject);
    if (subject?.role !== 'Collection') refusal('prepared collection subject');
    const prepared = collection(subject);
    if (prepared.mode !== binding.mode) refusal('prepared collection mode');
    return prepared;
  });
  const byCollection = new Map(collections.map(value => [value.subject.identity, value]));
  const fields = bindings.text_entries.map(identity => {
    const subject = subjects.get(identity);
    if (subject?.role !== 'TextEntry') refusal('prepared text subject');
    const action = actions.get(property(subject.identity, 'value-change-action')?.Identity);
    const value = typed(subject.identity, 'current-value');
    if (!action || action.target !== subject.identity || action.arguments.length !== 1
      || action.arguments[0].contract.value_kind !== 'value/text'
      || JSON.stringify(action.arguments[0].contract) !== JSON.stringify(value.contract)) refusal('text action association');
    const suggestionSource = property(subject.identity, 'suggestion-source', true)?.Identity;
    const policy = property(subject.identity, 'suggestion-policy', true)?.Text;
    const suggestions = suggestionSource ? byCollection.get(suggestionSource) : null;
    if (suggestionSource && (!suggestions || !['fixed', 'free'].includes(policy))) refusal('suggestion association');
    const options = suggestions?.items.map(item => ({ ...item, ...typed(item.identity, 'choice-value') })) ?? [];
    return { subject, action, value, options, policy };
  });
  const available = action => action.availability === 'Available';
  const argument = (action, bytes) => {
    if (action.arguments.length !== 1 || bytes.length > action.arguments[0].contract.maximum_bytes) refusal('argument capacity');
    return [{ name: action.arguments[0].name, value_kind: action.arguments[0].contract.value_kind, value: Array.from(bytes) }];
  };
  return { face, view, subjects, actions, fields, collections, property, typed, available, argument,
    textBytes(value, maximum) {
      if (typeof value !== 'string' || !value.isWellFormed()) refusal('invalid Unicode');
      const bytes = encoder.encode(value);
      if (bytes.length > maximum) refusal('text exceeds source byte bound');
      return bytes;
    },
  };
}

// Shared native-control reachability guard extracted from the existing owner
// component. Composed ancestors (including outside Shadow DOM) own inertness.
export function faceControlReachable(control, visibleControl = control) {
  if (!control.isConnected || control.matches(':disabled')) return false;
  for (let node = control; node; node = node.assignedSlot ?? node.parentElement ?? node.getRootNode().host) {
    if (node.inert || node.hidden) return false;
  }
  if (!control.checkVisibility({ visibilityProperty: true })
    || !visibleControl.checkVisibility({ visibilityProperty: true, opacityProperty: true })) return false;
  const bounds = visibleControl.getBoundingClientRect();
  return bounds.width > 0 && bounds.height > 0;
}
