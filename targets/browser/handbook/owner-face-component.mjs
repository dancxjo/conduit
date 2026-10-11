// A reusable browser realization of the checked owner Face. The SDK owns the
// Mask Plan, Play, Show acknowledgement and typed return route; this element
// owns only native controls and appearance.
const encoder = new TextEncoder();
const faceNode = (tag, text, className) => {
  const node = document.createElement(tag);
  node.textContent = text;
  if (className) node.className = className;
  return node;
};

export class OwnerFaceElement extends (globalThis.HTMLElement ?? class {}) {
  #surface;
  #invoke;
  #projectCollections;
  #disabled = true;
  constructor() {
    super();
    const shadow = this.attachShadow({ mode: 'open' });
    const look = document.createElement('link');
    look.rel = 'stylesheet';
    look.href = new URL('./owner-face-look.css', import.meta.url).href;
    this.#surface = document.createElement('div');
    shadow.append(look, this.#surface);
  }
  set disabled(value) {
    this.#disabled = Boolean(value);
    this.setAttribute('aria-busy', String(this.#disabled));
    for (const control of this.#surface.querySelectorAll('button, input, select')) {
      control.disabled = this.#disabled || control.dataset.unavailable === 'true';
    }
  }
  #ready(view, action) {
    return !this.#disabled && view.show_state === 'available'
      && view.interactions_admitted === true && action.availability === 'available';
  }
  show(view, { invoke, projectCollections = () => ({ collections: [], placed: new Set() }) } = {}) {
    this.#invoke = invoke;
    this.#projectCollections = projectCollections;
    this.#disabled = view.show_state !== 'available' || view.interactions_admitted !== true;

    const names = new Map(view.subjects.map(subject => [subject.identity, subject.name]));
    const documentNode = faceNode('article', '', 'owner-face-document');
    documentNode.setAttribute('part', 'document');
    const { collections, placed } = this.#projectCollections(view);
    const renderedActions = new Set();
    const actionReady = action => this.#ready(view, action);
    const actionControl = (action, itemName) => {
      renderedActions.add(action.identity);
      const control = document.createElement('form');
      control.setAttribute('part', 'action');
      control.dataset.ownerAction = action.identity;
      if (!itemName) control.append(faceNode('h5', action.name));
      const inputs = [];
      let supported = Array.isArray(action.arguments) && action.arguments.length <= 64;
      for (const argument of action.arguments ?? []) {
        const label = faceNode('label', argument.value_name);
        let input;
        if (Array.isArray(argument.choices) && argument.choices.length) {
          if (argument.choices.length > 64 || argument.choices.includes('')) {
            supported = false;
            continue;
          }
          input = document.createElement('select');
          const prompt = document.createElement('option');
          prompt.value = ''; prompt.textContent = 'Choose an option';
          input.append(prompt);
          for (const choice of argument.choices) {
            const option = document.createElement('option');
            option.value = choice; option.textContent = choice;
            input.append(option);
          }
          input.required = true;
        } else if (Array.isArray(argument.choices) && argument.value_kind === 'value/text'
          && Number.isSafeInteger(argument.maximum_bytes) && argument.maximum_bytes <= 4096) {
          input = document.createElement('input');
          input.type = 'text'; input.maxLength = argument.maximum_bytes;
          input.addEventListener('input', () => input.setCustomValidity(
            encoder.encode(input.value).length > argument.maximum_bytes
              ? `Use at most ${argument.maximum_bytes} UTF-8 bytes.` : ''));
        } else {
          supported = false;
          continue;
        }
        label.setAttribute('part', 'label');
        input.setAttribute('part', 'input');
        label.append(input);
        control.append(label);
        inputs.push({ name: argument.name, input });
      }
      const button = faceNode('button', itemName ? `${action.name} ${itemName}` : action.name);
      button.setAttribute('part', 'button');
      button.type = 'submit';
      button.disabled = !actionReady(action) || !supported || inputs.length !== action.arguments.length;
      button.dataset.unavailable = String(action.availability !== 'available' || !supported || inputs.length !== action.arguments.length);
      control.append(button);
      if (button.disabled) control.append(faceNode('p', action.explanation ??
        (supported ? 'This owner action is unavailable.' : 'This input has no supported browser form.')));
      control.addEventListener('submit', async event => {
        event.preventDefault();
        if (!this.#ready(view, action) || !this.#invoke) return;
        this.disabled = true;
        await this.#invoke({ view, action, arguments: inputs.map(({ name, input }) => ({ name, value: input.value })) });
      });
      return control;
    };
    const renderSubject = subject => {
      const node = document.createElement(subject.role === 'Body' || subject.role === 'Plot' ? 'article' : 'section');
      node.className = 'owner-face-subject';
      node.setAttribute('part', 'subject');
      node.dataset.faceRole = subject.role;
      node.append(faceNode('span', subject.semantic_role ?? subject.role, 'owner-face-role'));
      node.append(faceNode(subject.role === 'Body' ? 'h4' : 'h5', subject.name));
      for (const text of subject.text) node.append(faceNode('p', text));
      for (const value of subject.values ?? []) {
        const output = faceNode('p', value.text, 'owner-face-value');
        output.setAttribute('part', 'value');
        output.setAttribute('aria-label', value.name);
        node.append(output);
      }
      if (subject.properties.length) {
        const details = document.createElement('details');
        details.append(faceNode('summary', 'Exact facts'));
        const list = document.createElement('ul');
        for (const property of subject.properties) {
          const item = document.createElement('li'); item.textContent = property; list.append(item);
        }
        details.append(list); node.append(details);
      }
      for (const action of view.actions.filter(action => action.target === subject.identity
        && action.disclosure === 'CurrentAction')) node.append(actionControl(action));
      placed.add(subject.identity);
      return node;
    };
    for (const { collection, open, completed, count } of collections) {
      const section = document.createElement('section');
      section.className = 'owner-face-collection';
      section.setAttribute('part', 'collection');
      section.append(faceNode('h4', collection.name));
      for (const text of collection.text) section.append(faceNode('p', text));
      section.append(faceNode('p', count, 'owner-face-collection-count'));
      const openList = document.createElement('ol');
      openList.className = 'owner-face-items';
      openList.setAttribute('aria-label', `Things left in ${collection.name}`);
      for (const item of open) {
        const row = document.createElement('li');
        row.append(faceNode('span', item.name, 'owner-face-item-name'));
        for (const action of view.actions.filter(action => action.target === item.identity
          && actionReady(action) && action.disclosure === 'CurrentAction')) {
          row.append(actionControl(action, item.name));
        }
        openList.append(row);
      }
      if (open.length) section.append(openList);
      else section.append(faceNode('p', 'Nothing left to do.', 'owner-face-empty'));
      if (completed.length) {
        const details = document.createElement('details');
        details.className = 'owner-face-completed';
        details.append(faceNode('summary', `Show ${completed.length} completed ${completed.length === 1 ? 'item' : 'items'}`));
        const completedList = document.createElement('ol');
        completedList.className = 'owner-face-items';
        completedList.setAttribute('aria-label', `Completed items in ${collection.name}`);
        for (const item of completed) {
          const row = document.createElement('li');
          row.append(faceNode('span', item.name, 'owner-face-item-name'));
          for (const action of view.actions.filter(action => action.target === item.identity
            && actionReady(action) && action.disclosure === 'CurrentAction')) {
            row.append(actionControl(action, item.name));
          }
          completedList.append(row);
        }
        details.append(completedList); section.append(details);
      }
      for (const action of view.actions.filter(action => action.target === collection.identity
        && actionReady(action) && action.disclosure === 'CurrentAction')) {
        section.append(actionControl(action));
      }
      documentNode.append(section);
    }
    const inspection = document.createElement('details');
    inspection.append(faceNode('summary', 'Inspect other Face subjects'));
    for (const group of view.subjects) {
      if (group.semantic_role !== 'conduit.presentation/choice-group@1'
        || group.choice_multiplicity !== 'exclusive') continue;
      const options = view.relationships
        .filter(relation => relation.kind === 'Contains' && relation.source === group.identity)
        .map(relation => view.subjects.find(subject => subject.identity === relation.target));
      // A radio group requires exact semantic selection flags and one offered
      // zero-input action per option. Unfamiliar contracts keep the generic view.
      const offered = options.map(option => view.actions.filter(action => action.target === option?.identity));
      if (!options.length || options.some((option, index) => !option
        || option.flags?.filter(flag => flag.name === 'selected' && typeof flag.value === 'boolean').length !== 1
        || offered[index].length !== 1 || offered[index][0].arguments.length !== 0)
        || options.filter(option => option.flags.find(flag => flag.name === 'selected').value).length > 1) continue;
      const fieldset = document.createElement('fieldset');
      fieldset.className = 'owner-face-choice-group';
      fieldset.setAttribute('part', 'choice-group');
      fieldset.append(faceNode('legend', group.name));
      for (const text of group.text) fieldset.append(faceNode('p', text));
      const choices = document.createElement('div');
      choices.className = 'owner-face-choices';
      options.forEach((option, index) => {
        const action = offered[index][0];
        const label = faceNode('label', '', 'owner-face-choice');
        const input = document.createElement('input');
        label.setAttribute('part', 'choice');
        input.setAttribute('part', 'input');
        input.type = 'radio';
        input.name = group.identity;
        input.value = option.identity;
        input.checked = option.flags.find(flag => flag.name === 'selected').value;
        input.disabled = !actionReady(action);
        input.dataset.unavailable = String(action.availability !== 'available');
        label.dataset.ownerAction = action.identity;
        renderedActions.add(action.identity);
        label.append(input, faceNode('span', option.name));
        if (action.explanation) label.append(faceNode('small', action.explanation));
        input.addEventListener('change', async () => {
          if (!this.#ready(view, action) || !this.#invoke) return;
          // Selection remains the acknowledged Face's fact until a fresh Show.
          choices.querySelectorAll('input').forEach((control, optionIndex) => {
            control.checked = options[optionIndex].flags.find(flag => flag.name === 'selected').value;
          });
          this.disabled = true;
          await this.#invoke({ view, action, arguments: [] });
        });
        choices.append(label);
        placed.add(option.identity);
      });
      fieldset.append(choices);
      documentNode.append(fieldset);
      placed.add(group.identity);
    }
    for (const subject of view.subjects) {
      if (placed.has(subject.identity)) continue;
      const main = ['Primary', 'Context'].includes(subject.disclosure)
        && (!collections.length || subject.role === 'Status');
      (main ? documentNode : inspection).append(renderSubject(subject));
    }
    if (inspection.children.length > 1) documentNode.append(inspection);
    if (view.relationships.length) {
      const relations = document.createElement('details');
      relations.append(faceNode('summary', 'How these parts relate'));
      const list = document.createElement('ul');
      for (const relationship of view.relationships) {
        const item = document.createElement('li');
        item.textContent = `${names.get(relationship.source) ?? relationship.source} ${relationship.kind.toLowerCase()} ${names.get(relationship.target) ?? relationship.target}`;
        list.append(item);
      }
      relations.append(list); documentNode.append(relations);
    }
    if (view.actions.length) {
      const actions = document.createElement('section');
      actions.append(faceNode('h4', 'What you can do'));
      const secondary = document.createElement('details');
      secondary.append(faceNode('summary', 'More actions and unavailable controls'));
      for (const action of view.actions) {
        if (renderedActions.has(action.identity)) continue;
        const container = actionReady(action)
          && action.disclosure === 'CurrentAction' ? actions : secondary;
        container.append(actionControl(action));
      }
      if (actions.children.length > 1) documentNode.append(actions);
      if (secondary.children.length > 1) documentNode.append(secondary);
    }
    this.#surface.replaceChildren(documentNode);
    this.dataset.faceId = view.face_id;
    this.dataset.faceRevision = view.face_revision;
    this.dataset.showId = view.show_id;
    this.disabled = this.#disabled;
  }
}
if (globalThis.customElements && !customElements.get('conduit-owner-face')) {
  customElements.define('conduit-owner-face', OwnerFaceElement);
}
