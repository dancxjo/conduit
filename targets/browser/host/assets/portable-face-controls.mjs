import { portableFaceModel, faceControlReachable } from "./portable-face-model.mjs";
import { attachPortableSuggestions } from "./portable-face-combobox.mjs";

const node = (tag, text = '') => { const element = document.createElement(tag); element.textContent = text; return element; };
const reason = action => action.availability?.Unavailable?.explanation ?? action.availability?.Refused?.explanation ?? 'This source action is unavailable.';
let surfaceSequence = 0;

// One browser Back's DOM preparation over original Face references. Input drafts
// and popup navigation stay local; the trusted Host owns Show and submission.
export function renderPortableFace(root, encounter, { enhanced = false, submit, onRefusal = () => {} } = {}) {
  const model = portableFaceModel(encounter);
  if (typeof submit !== 'function' || ++surfaceSequence > 0xffff_ffff) throw new Error('Portable Face Host binding is absent');
  const prefix = `conduit-controls-${surfaceSequence}`;
  const article = node('article'); article.className = 'portable-face';
  article.setAttribute('aria-label', model.face.subjects[0]?.name ?? 'Conduit Face');
  article.dataset.faceId = model.face.identity; article.dataset.faceRevision = String(model.face.revision);
  const status = node('p'); status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite');
  const controls = [], required = [], placed = new Set(), fieldCollections = new Set();
  let active = false, retired = false, pending = false;
  const refreshDisabled = () => {
    for (const control of controls) control.disabled = !active || retired || pending || control.dataset.sourceUnavailable === 'true';
    article.setAttribute('aria-busy', String(pending));
  };
  const register = (control, action) => {
    control.dataset.sourceUnavailable = String(!model.available(action));
    control.dataset.sourceAction = action.identity; controls.push(control);
    return control;
  };
  const invoke = async (event, action, arguments_) => {
    if (!event.isTrusted || !active || retired || pending || !model.available(action)) return;
    if (!article.isConnected || !event.target?.isConnected) return;
    try { inspect(); } catch (error) { active = false; refreshDisabled(); report(error); return; }
    pending = true; refreshDisabled();
    try { await submit({ action, arguments: arguments_, view: model.view }); }
    catch (error) { status.textContent = error.message ?? String(error); onRefusal(error); }
    // A submitted encounter cannot authorize another action, even after refusal.
    finally { active = false; pending = false; refreshDisabled(); }
  };
  const report = error => { status.textContent = error.message ?? String(error); onRefusal(error); };
  const heading = node('h2', model.face.subjects[0]?.name ?? 'Conduit Face'); article.append(heading);

  for (const [index, field] of model.fields.entries()) {
    const form = node('form'); form.className = 'portable-face-field';
    const id = `${prefix}-field-${index}`;
    const label = node('label', field.subject.name); label.htmlFor = id;
    let input, picker;
    const maximum = field.action.arguments[0].contract.maximum_bytes;
    if (!enhanced && field.policy === 'fixed') {
      input = node('select');
      for (const option of field.options) {
        const entry = node('option', option.name); entry.value = option.identity;
        entry.selected = option.bytes.length === field.value.bytes.length && option.bytes.every((byte, i) => byte === field.value.bytes[i]);
        input.append(entry);
      }
    } else {
      input = node('input'); input.type = 'text'; input.value = field.value.text;
      input.maxLength = maximum;
    }
    input.id = id; input.setAttribute('aria-label', field.subject.name);
    input.dataset.sourceSubject = field.subject.identity;
    register(input, field.action); form.append(label, input);
    if (enhanced && field.policy) {
      picker = attachPortableSuggestions(input, form, {
        options: field.options, policy: field.policy, currentBytes: field.value.bytes, id,
        onDraft: () => { input.setCustomValidity(''); },
      });
    } else if (field.policy === 'free' && field.options.length) {
      const suggestions = node('select'); suggestions.setAttribute('aria-label', `${field.subject.name} suggestions`);
      const prompt = node('option', 'Choose a suggestion for the draft'); prompt.value = ''; suggestions.append(prompt);
      for (const option of field.options) { const entry = node('option', option.name); entry.value = option.identity; suggestions.append(entry); }
      register(suggestions, field.action);
      suggestions.addEventListener('change', event => {
        if (!event.isTrusted || suggestions.disabled) return;
        const option = field.options.find(option => option.identity === suggestions.value);
        if (option) { input.value = option.text; input.setCustomValidity(''); }
      });
      form.append(suggestions);
    }
    if (field.policy) {
      const source = model.property(field.subject.identity, 'suggestion-source').Identity;
      fieldCollections.add(source);
      const note = node('p', field.policy === 'fixed'
        ? 'Choose a declared option, then submit. Typing only searches the options.'
        : 'Type your own value or choose a suggestion, then submit.');
      note.id = `${id}-help`; input.setAttribute('aria-describedby', note.id); form.append(note);
    }
    const save = register(node('button', field.action.name), field.action); save.type = 'submit';
    const cancel = register(node('button', 'Cancel draft'), field.action); cancel.type = 'button';
    const commit = event => {
      if (!event.isTrusted || input.disabled || picker?.composing()) return;
      try {
        let bytes;
        if (picker) bytes = picker.bytes(model, maximum);
        else if (field.policy === 'fixed') {
          const option = field.options.find(option => option.identity === input.value);
          if (!option) throw new Error('Choose a declared option');
          bytes = option.bytes;
        } else bytes = model.textBytes(input.value, maximum);
        input.setCustomValidity('');
        void invoke(event, field.action, model.argument(field.action, bytes));
      } catch (error) { input.setCustomValidity(error.message); input.reportValidity(); report(error); }
    };
    save.addEventListener('click', event => { event.preventDefault(); commit(event); });
    // Native text/caret/selection editing remains with the browser. Popup Enter
    // is consumed by its local candidate picker and never reaches this handler.
    form.addEventListener('keydown', event => {
      if (event.key === 'Enter' && event.target === input && input instanceof HTMLInputElement
        && !event.defaultPrevented && !event.isComposing && !picker?.composing()) {
        event.preventDefault(); commit(event);
      }
    });
    form.addEventListener('submit', event => event.preventDefault());
    cancel.addEventListener('click', event => {
      if (!event.isTrusted || cancel.disabled) return;
      if (picker) picker.reset();
      else if (field.policy === 'fixed') {
        const option = field.options.find(option => option.bytes.length === field.value.bytes.length && option.bytes.every((byte, i) => byte === field.value.bytes[i]));
        input.value = option?.identity ?? '';
      } else input.value = field.value.text;
      input.setCustomValidity(''); status.textContent = 'Draft cancelled. Source value is unchanged.';
    });
    form.append(save, cancel);
    if (!model.available(field.action)) form.append(node('p', reason(field.action)));
    required.push(input, save); placed.add(field.action.identity); article.append(form);
  }

  for (const collection of model.collections) {
    if (fieldCollections.has(collection.subject.identity)) continue;
    const section = node('section'); section.className = 'portable-face-collection';
    section.dataset.sourceCollection = collection.subject.identity;
    section.append(node('h3', collection.subject.name));
    const list = node('ul');
    for (const item of collection.items) {
      const row = node('li'); row.dataset.sourceItem = item.identity;
      if (collection.mode === 'independent') {
        const action = model.actions.get(model.property(item.identity, 'boolean-change-action')?.Identity);
        const selected = model.property(item.identity, 'selected')?.Flag;
        if (!action || action.target !== item.identity || action.arguments.length !== 1
          || action.arguments[0].contract.value_kind !== 'value/bool' || typeof selected !== 'boolean') throw new Error('Independent choice binding refused');
        const label = node('label'); const checkbox = register(node('input'), action);
        checkbox.type = 'checkbox'; checkbox.checked = selected;
        checkbox.setAttribute('aria-label', item.name); label.append(checkbox, node('span', item.name));
        checkbox.addEventListener('change', event => {
          if (!event.isTrusted || checkbox.disabled) { checkbox.checked = selected; return; }
          void invoke(event, action, model.argument(action, [Number(checkbox.checked)]));
        });
        row.append(label); if (!model.available(action)) row.append(node('p', reason(action)));
        placed.add(action.identity); required.push(checkbox);
      } else row.append(node('span', item.name));
      list.append(row);
    }
    if (!collection.items.length) section.append(node('p', 'No items.'));
    section.append(list); article.append(section);
  }

  for (const action of model.face.actions) {
    if (placed.has(action.identity)) continue;
    if (action.arguments.length) {
      const unsupported = node('p', `${action.name}: this browser cannot prepare its typed input.`);
      unsupported.dataset.hostRefusal = 'UnsupportedTypedInput'; article.append(unsupported); continue;
    }
    const button = register(node('button', action.name), action); button.type = 'button';
    button.addEventListener('click', event => { void invoke(event, action, []); });
    required.push(button); article.append(button);
    if (!model.available(action)) article.append(node('p', reason(action)));
  }
  for (const subject of model.face.subjects.filter(subject => ['Status', 'Alert'].includes(subject.role))) {
    const message = node('p', model.face.text.filter(text => text.subject === subject.identity).map(text => text.text).join(' '));
    message.dataset.sourceStatus = subject.identity; message.setAttribute('role', subject.role === 'Alert' ? 'alert' : 'status'); article.append(message);
  }
  const details = node('details'), summary = node('summary', 'Exact source facts'); details.append(summary);
  for (const subject of model.face.subjects) {
    const section = node('section'); section.append(node('h4', subject.name));
    for (const text of model.face.text.filter(text => text.subject === subject.identity)) section.append(node('p', text.text));
    details.append(section);
  }
  article.append(details, status); root.replaceChildren(article); refreshDisabled();
  function inspect() {
      if (!article.isConnected || required.some(control => {
        const bounds = control.getBoundingClientRect();
        for (let ancestor = control; ancestor; ancestor = ancestor.assignedSlot ?? ancestor.parentElement ?? ancestor.getRootNode().host) {
          if (ancestor.inert || ancestor.hidden) return true;
        }
        return !control.isConnected || !control.checkVisibility({ opacityProperty: true, visibilityProperty: true }) || bounds.width <= 0 || bounds.height <= 0
          || (active && !control.disabled && !faceControlReachable(control));
      })) throw new Error('A required source control is inaccessible');
      if (article.querySelectorAll('*').length > 2048) throw new Error('Component node capacity');
      return { controls: required.length, nodes: article.querySelectorAll('*').length };
    }
  return {
    activate(current) {
      if (retired || current.face.identity !== model.face.identity || String(current.face.revision) !== model.view.face_revision
        || current.view.show_id !== model.view.show_id || current.view.mask_plan_id !== model.view.mask_plan_id
        || current.view.show_state !== 'available' || current.view.interactions_admitted !== true) throw new Error('Acknowledged Show binding refused');
      active = true; refreshDisabled();
    },
    inspect,
    retire() { retired = true; active = false; refreshDisabled(); },
    sourceFace: model.face.identity,
  };
}
