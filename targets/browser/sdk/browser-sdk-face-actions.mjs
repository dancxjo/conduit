// The DOM supplies text values; the checked Face supplies names, contracts,
// availability and the exact current action. WASM validates the bytes again.
const encoder = new TextEncoder();

export function checkedAcknowledgedOwnerFaceArguments(acknowledged, view, actionId, target, supplied) {
  if (acknowledged?.show_state !== 'available' || acknowledged.interactions_admitted !== true
    || view?.show_state !== 'available' || view.interactions_admitted !== true
    || ['body_id', 'show_id', 'face_id', 'face_revision'].some(key => view[key] !== acknowledged[key])) {
    throw new Error('Current acknowledged owner Show is required for interaction.');
  }
  // Caller-owned view objects cannot replace the acknowledged input contract.
  return checkedOwnerFaceArguments(acknowledged, actionId, target, supplied);
}

export function checkedOwnerFaceArguments(view, actionId, target, supplied) {
  const action = view?.actions?.find(candidate => candidate.identity === actionId);
  if (!action || action.target !== target || action.availability !== 'available'
    || !Array.isArray(action.arguments) || action.arguments.length > 64
    || !Array.isArray(supplied) || supplied.length !== action.arguments.length) {
    throw new Error('Choose one currently available Face action and its declared inputs.');
  }
  const declarations = new Map();
  for (const declaration of action.arguments) {
    if (typeof declaration?.name !== 'string' || !declaration.name.trim() || declarations.has(declaration.name)) {
      throw new Error('Face action input declaration is missing or duplicated.');
    }
    declarations.set(declaration.name, declaration);
  }
  const names = new Set();
  return supplied.map(argument => {
    if (typeof argument?.name !== 'string' || names.has(argument.name)) {
      throw new Error('Face action input is missing or duplicated.');
    }
    names.add(argument.name);
    const declaration = declarations.get(argument.name);
    if (typeof argument.value === 'string' && !argument.value.isWellFormed()) {
      throw new Error('Face action input must be valid Unicode text.');
    }
    if (!declaration || typeof argument.value !== 'string'
      || !Number.isSafeInteger(declaration.maximum_bytes)
      || declaration.maximum_bytes < 0 || declaration.maximum_bytes > 4096
      || encoder.encode(argument.value).length > declaration.maximum_bytes) {
      throw new Error('Face action input differs from its declared bound.');
    }
    const choices = declaration.choices;
    if (!Array.isArray(choices) || choices.length > 64
      || choices.some(choice => typeof choice !== 'string' || choice === '' || !choice.isWellFormed()
        || encoder.encode(choice).length > declaration.maximum_bytes)
      || (choices.length ? !choices.includes(argument.value)
        : declaration.value_kind !== 'value/text')) {
      throw new Error('This Face action input has no supported browser text form.');
    }
    return { name: argument.name, value: argument.value };
  });
}
