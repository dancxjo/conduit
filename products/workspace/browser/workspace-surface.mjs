// Choose interaction affordances from the exact foreground partition when planned.
export function configureWorkspaceInput(input, form, partition) {
  const placements = partition?.plan.fragments.flatMap(fragment => fragment.placements) ?? [];
  const kinds = new Set(partition ? placements.map(placement => placement.kind_id) : form?.required_kinds ?? []);
  const keyboard = kinds.has('input/keyboard'), pointer = kinds.has('input/pointer-source'), button = kinds.has('input/button');
  const acceptsInput = keyboard || pointer || button;
  const hasOutput = placements.some(placement => placement.resources.some(resource => resource.class_id === 'conduit.resource/presentation-slot@1'));
  input.hidden = !form || (!acceptsInput && !hasOutput);
  input.dataset.acceptsInput = String(acceptsInput);
  input.setAttribute('role', keyboard ? 'textbox' : 'group');
  if (keyboard) input.setAttribute('aria-multiline', 'true');
  else input.removeAttribute('aria-multiline');
  input.setAttribute('aria-label', `${acceptsInput ? 'Interact with' : 'Output of'} ${form?.title ?? 'your Form'}`);
  input.tabIndex = input.disabled || !acceptsInput ? -1 : 0;
  const prompt = input.querySelector('.input-prompt');
  prompt.hidden = !acceptsInput;
  prompt.textContent = pointer ? 'Choose a position here.' : button ? 'Press here.' : 'Type to begin.';
  if (kinds.has('text/submit-lines')) return 'Type a message. Press Enter to send.';
  if (keyboard) return 'Type something. Your Form is listening.';
  if (pointer) return 'Click a horizontal position to choose a pitch.';
  if (button) return 'Press and release here. Your Form is listening.';
  if (kinds.has('sound/startup-chime')) return 'This Form makes a short sound when eligible. Its playback outcome is in lifecycle evidence.';
  return form ? 'Watch this Form take shape.' : 'Your Body is retained without running Forms.';
}
