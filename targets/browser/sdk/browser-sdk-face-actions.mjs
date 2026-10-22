// The DOM supplies text values; the checked Face supplies names, contracts,
// availability and the exact current action. WASM validates the bytes again.
const encoder = new TextEncoder();

// Adapter admission retains the same finite refusal vocabulary as Face.
const refuse = (code, message) => {
  const error = new Error(message);
  error.code = code;
  throw error;
};

// This ingress realizes only the resident Patchbay command Fore. Body lifecycle
// and other contributors' actions retain their own admitted return routes.
export function checkedPatchbayFaceArguments(acknowledged, view, actionId, target, supplied) {
  const checked = checkedAcknowledgedOwnerFaceArguments(acknowledged, view, actionId, target, supplied);
  const action = acknowledged.actions.find(candidate => candidate.identity === actionId);
  if (!['patchbay/select-plot@1', 'patchbay/edit-subject@1', 'patchbay/change-masks@1'].includes(action.intent)) {
    refuse('UnsupportedAction', 'This action has no resident Patchbay return route.');
  }
  return checked;
}

export function checkedAcknowledgedOwnerFaceArguments(acknowledged, view, actionId, target, supplied) {
  if (acknowledged?.show_state !== 'available' || acknowledged.interactions_admitted !== true
    || view?.show_state !== 'available' || view.interactions_admitted !== true
    || ['body_id', 'show_id', 'face_id', 'face_revision'].some(key => view[key] !== acknowledged[key])) {
    refuse('StaleShow', 'Current acknowledged owner Show is required for interaction.');
  }
  // Caller-owned view objects cannot replace the acknowledged input contract.
  return checkedOwnerFaceArguments(acknowledged, actionId, target, supplied);
}

export function checkedOwnerFaceArguments(view, actionId, target, supplied) {
  const action = view?.actions?.find(candidate => candidate.identity === actionId);
  if (!action || action.target !== target || action.availability !== 'available'
    || !Array.isArray(action.arguments) || action.arguments.length > 64
    || !Array.isArray(supplied) || supplied.length !== action.arguments.length) {
    refuse(!action ? 'UnknownAction' : action.target !== target ? 'WrongTarget'
      : action.availability !== 'available' ? (action.availability === 'refused' ? 'RefusedAction' : 'UnavailableAction')
        : Array.isArray(supplied) && supplied.length <= 64 && new Set(supplied.map(argument => argument?.name)).size !== supplied.length ? 'DuplicateArgument'
        : Array.isArray(supplied) && supplied.length < (action.arguments?.length ?? 0) ? 'MissingArgument' : 'UnknownArgument',
    'Choose one currently available Face action and its declared inputs.');
  }
  const declarations = new Map();
  for (const declaration of action.arguments) {
    if (typeof declaration?.name !== 'string' || !declaration.name.trim() || declarations.has(declaration.name)) {
      refuse('InvalidFace', 'Face action input declaration is missing or duplicated.');
    }
    declarations.set(declaration.name, declaration);
  }
  const names = new Set();
  return supplied.map(argument => {
    if (typeof argument?.name !== 'string' || names.has(argument.name)) {
      refuse(typeof argument?.name !== 'string' ? 'UnknownArgument' : 'DuplicateArgument',
        'Face action input is missing or duplicated.');
    }
    names.add(argument.name);
    const declaration = declarations.get(argument.name);
    if (typeof argument.value === 'string' && !argument.value.isWellFormed()) {
      refuse('MalformedEncoding', 'Face action input must be valid Unicode text.');
    }
    if (!declaration || typeof argument.value !== 'string'
      || !Number.isSafeInteger(declaration.maximum_bytes)
      || declaration.maximum_bytes < 0 || declaration.maximum_bytes > 4096
      || encoder.encode(argument.value).length > declaration.maximum_bytes) {
      refuse(!declaration ? 'UnknownArgument' : typeof argument.value !== 'string' ? 'WrongValueKind' : 'OversizeValue',
        'Face action input differs from its declared bound.');
    }
    const choices = declaration.choices;
    if (!Array.isArray(choices) || choices.length > 64
      || choices.some(choice => typeof choice !== 'string' || choice === '' || !choice.isWellFormed()
        || encoder.encode(choice).length > declaration.maximum_bytes)
      || (choices.length ? !choices.includes(argument.value)
        : declaration.value_kind !== 'value/text')) {
      refuse('ViolatedConstraint', 'This Face action input has no supported browser text form.');
    }
    return { name: argument.name, value: argument.value };
  });
}
