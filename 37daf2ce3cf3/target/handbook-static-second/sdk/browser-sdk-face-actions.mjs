// The DOM supplies text values; the checked Face supplies names, contracts,
// availability and the exact current action. WASM validates the bytes again.
const encoder = new TextEncoder();

export function checkedOwnerFaceArguments(view, actionId, target, supplied) {
  const action = view?.actions?.find(candidate => candidate.identity === actionId);
  if (!action || action.target !== target || action.availability !== 'available'
    || !Array.isArray(action.arguments) || action.arguments.length > 64
    || !Array.isArray(supplied) || supplied.length !== action.arguments.length) {
    throw new Error('Choose one currently available Face action and its declared inputs.');
  }
  const names = new Set();
  return supplied.map(argument => {
    if (typeof argument?.name !== 'string' || names.has(argument.name)) {
      throw new Error('Face action input is missing or duplicated.');
    }
    names.add(argument.name);
    const declaration = action.arguments.find(candidate => candidate.name === argument.name);
    if (!declaration || typeof argument.value !== 'string'
      || !Number.isSafeInteger(declaration.maximum_bytes)
      || declaration.maximum_bytes < 0 || declaration.maximum_bytes > 4096
      || encoder.encode(argument.value).length > declaration.maximum_bytes) {
      throw new Error('Face action input differs from its declared bound.');
    }
    const choices = declaration.choices;
    if (!Array.isArray(choices) || choices.length > 64
      || (choices.length ? !choices.includes(argument.value)
        : declaration.value_kind !== 'value/text')) {
      throw new Error('This Face action input has no supported browser text form.');
    }
    return { name: argument.name, value: argument.value };
  });
}
