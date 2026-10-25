import { decorate } from "./owner-face-icons.mjs";

// Only existing checked presets cross the owner transport. These controls hold
// proposals; the acknowledged component remains the rendering authority.
const presets = Object.freeze({
  'compact/phosphor': 'compact', 'compact/editorial': 'compact-editorial',
  'spacious/phosphor': 'spacious-phosphor', 'spacious/editorial': 'spacious',
});
const recipes = Object.freeze({
  'conduit.face/environment-history@1': 'environment-history',
  'conduit.face/patchbay-nodes@1': 'patchbay-nodes',
  'conduit.face/todo-flat@1': 'flat',
  'conduit.face/todo-grouped@1': 'grouped',
  'conduit.face/todo-required-item@1': 'required-item',
});
let controlsSequence = 0;

export function createOwnerLookControls(onApply) {
  const prefix = `owner-look-${++controlsSequence}-`;
  const element = document.createElement('fieldset');
  element.className = 'owner-look-controls';
  const legend = decorate(document.createElement('legend'), 'Face');
  legend.append('View appearance');
  const fields = document.createElement('div'); fields.className = 'owner-look-fields';
  const select = (caption, choices) => {
    const field = document.createElement('div'); field.className = 'owner-look-field';
    const label = document.createElement('label'); label.append(caption);
    const input = document.createElement('select');
    input.id = prefix + fields.children.length; label.htmlFor = input.id;
    for (const [value, name] of choices) {
      const option = document.createElement('option'); option.value = value;
      option.textContent = name; input.append(option);
    }
    field.append(label, input); fields.append(field); return input;
  };
  const density = select('Density', [['compact', 'Compact'], ['spacious', 'Spacious']]);
  const palette = select('Palette', [['phosphor', 'Phosphor · dark'], ['editorial', 'Editorial · light']]);
  const recipe = select('Layout', [['', 'Choose layout'], ['flat', 'Single section'],
    ['grouped', 'Grouped sections'], ['required-item', 'Bound item'], ['patchbay-nodes', 'Patchbay nodes'],
    ['environment-history', 'Recorded conditions graph']]);
  const historyOption = [...recipe.options].find(option => option.value === 'environment-history');
  historyOption.disabled = true;
  let historyUnit;
  const minimum = select('Minimum text size', [[100, '100%'], [150, '150%'], [200, '200%']]);
  const apply = decorate(document.createElement('button'), 'Confirm');
  apply.type = 'button'; apply.append('Apply face');
  const feedback = document.createElement('p'); feedback.dataset.lookFeedback = '';
  feedback.setAttribute('role', 'status');
  feedback.textContent = 'Changes apply after the owner accepts a fresh Show.';
  const note = document.createElement('p'); note.className = 'owner-look-note';
  note.textContent = 'Choose spacing and color. Each Look also supplies its typography; layout is a separate choice.';
  element.append(legend, note, fields, apply, feedback);
  apply.addEventListener('click', () => {
    if (element.disabled || apply.disabled) return;
    const look = presets[`${density.value}/${palette.value}`];
    if (!look || !Object.values(recipes).includes(recipe.value)
        || !['100', '150', '200'].includes(minimum.value)) {
      feedback.textContent = 'Choose a supported appearance and layout.';
      return;
    }
    if (recipe.value === 'environment-history' && (historyOption.disabled
        || look !== 'compact-editorial' || (historyUnit !== undefined && historyUnit !== '°C'))) {
      feedback.textContent = 'History appearance selection currently requires its compact editorial Celsius view.';
      return;
    }
    onApply({ look, recipe: recipe.value, minimumTextScalePercent: Number(minimum.value) });
  });
  const setDisabled = value => { element.disabled = Boolean(value); apply.disabled = Boolean(value); };
  setDisabled(true);
  let observedPolicy;
  return {
    element, apply, setDisabled,
    observe(view) {
      historyOption.disabled = !view?.subjects?.some(subject => subject.identity === 'environment-history'
        && subject.role === 'Collection');
      historyUnit = view?.component?.history?.temperature_unit;
      const expression = view?.component?.expression?.expression;
      // New Face/Show occurrences do not erase an unfinished local proposal.
      // Reconcile fields only when their acknowledged policy actually changes.
      const policy = JSON.stringify([view?.body_id, view?.component?.recipe_identity, historyUnit,
        expression?.density, expression?.palette, expression?.typography,
        expression?.text_scale_percent,
        view?.component?.expression?.accessibility_overrides?.text_scale]);
      if (policy === observedPolicy) return;
      observedPolicy = policy;
      if (expression && Object.hasOwn(presets, `${expression.density}/${expression.palette}`)) {
        density.value = expression.density; palette.value = expression.palette;
        feedback.textContent = `Shown: ${expression.density} · ${expression.palette} · ${expression.typography ?? 'default'} type · ${expression.text_scale_percent}% text.`;
      } else {
        feedback.textContent = 'The current view uses its default appearance. Choose a layout before applying a Look.';
      }
      const identity = view?.component?.recipe_identity;
      recipe.value = typeof identity === 'string' && Object.hasOwn(recipes, identity) ? recipes[identity] : '';
      if (view?.component?.expression?.accessibility_overrides?.text_scale
          && ['150', '200'].includes(String(expression?.text_scale_percent))) {
        minimum.value = String(expression.text_scale_percent);
      }
    },
  };
}
