// Trusted DOM presentation of the checked owner Face. The SDK retains the
// Mask Plan, Play, Show acknowledgement and typed return route. This controller
// holds only controls, appearance and provisional local interaction state.
const encoder = new TextEncoder();
const argumentValidity = (value, maximum) => !value.isWellFormed() ? 'Use valid Unicode text.'
  : encoder.encode(value).length > maximum ? `Use at most ${maximum} UTF-8 bytes.` : '';
const exactChoiceLabel = value => JSON.stringify(value).replace(/[\s\u007f]/gu,
  character => `\\u${character.charCodeAt(0).toString(16).padStart(4, '0')}`);
const faceNode = (tag, text, className) => {
  const node = document.createElement(tag);
  node.textContent = text;
  if (className) node.className = className;
  return node;
};

export class OwnerFacePresentation {
  #element;
  #surface;
  #invoke;
  #projectCollections;
  #supportsAction = () => true;
  #disabled = true;
  #dispatched = false;
  #pendingFocus;
  #render;
  #layoutObserver;
  constructor(element, surface) {
    this.#element = element;
    this.#surface = surface;
  }
  retire() {
    this.#layoutObserver?.disconnect();
    // A removed surface no longer owns an active encounter. Reattachment or
    // re-enabling controls must not revive its old event-to-action bindings.
    this.#render = undefined;
    this.#dispatched = true;
    this.disabled = true;
    this.#pendingFocus = undefined;
  }
  set disabled(value) {
    const focused = value ? this.#focusedState() : undefined;
    if (focused) this.#pendingFocus = focused;
    this.#disabled = Boolean(value);
    this.#element.setAttribute('aria-busy', String(this.#disabled));
    for (const control of this.#surface.querySelectorAll('button, input, select')) {
      control.disabled = this.#disabled || control.dataset.unavailable === 'true';
    }
  }
  #focusedState() {
    const control = (this.#element.shadowRoot ?? this.#element.ownerDocument).activeElement;
    if (!control?.dataset.ownerFocus || !this.#surface.contains(control)) return;
    return { key: control.dataset.ownerFocus, value: control.value,
      start: control.selectionStart, end: control.selectionEnd, direction: control.selectionDirection };
  }
  #ready(view, action, render) {
    return render === this.#render && !this.#disabled && view.show_state === 'available'
      && view.interactions_admitted === true && action.availability === 'available'
      && this.#supportsAction(action);
  }
  #reachable(control, visibleControl = control) {
    // Native focus/activation requires a visible, enabled control. This is a
    // renderer guard; the SDK and owner still check interaction authority.
    if (!this.#element.isConnected || control.matches(':disabled')) return false;
    // Inertness follows the composed tree, including slotted content and
    // ancestors outside this element's shadow root. It does not hide boxes.
    for (let node = control; node; node = node.assignedSlot ?? node.parentElement ?? node.getRootNode().host) {
      if (node.inert || node.hidden) return false;
    }
    // Custom radio inputs are transparent inside their visible native label.
    if (!control.checkVisibility({ visibilityProperty: true })
      || !visibleControl.checkVisibility({ visibilityProperty: true, opacityProperty: true })) return false;
    const bounds = visibleControl.getBoundingClientRect();
    return bounds.width > 0 && bounds.height > 0;
  }
  show(view, { invoke, projectCollections = () => ({ collections: [], placed: new Set() }),
    supportsAction = () => true } = {}) {
    const focusState = this.#focusedState() ?? this.#pendingFocus;
    const focusedKey = focusState?.key;
    const disclosures = new Map();
    if (this.#render && typeof view.body_id === 'string' && view.body_id) {
      for (const node of this.#surface.querySelectorAll('details[data-owner-disclosure]')) {
        disclosures.set(node.dataset.ownerDisclosure, node.open);
      }
    }
    const localDetails = (subject, kind, label) => {
      const details = document.createElement('details');
      const key = JSON.stringify([view.body_id, 'details', subject, kind]);
      details.dataset.ownerDisclosure = key;
      details.open = disclosures.get(key) ?? false;
      const summary = faceNode('summary', label);
      summary.dataset.ownerFocus = key;
      details.append(summary);
      return details;
    };
    const drafts = new Map();
    // Temporary refresh/preparation disables controls without submitting them.
    // A dispatched or disconnected encounter starts with fresh input.
    if (!this.#dispatched && typeof view.body_id === 'string' && view.body_id) {
      for (const input of this.#surface.querySelectorAll('[data-owner-draft]')) {
        if (input.dataset.unavailable !== 'true') drafts.set(input.dataset.ownerDraft, input.value);
      }
    }
    const render = this.#render = {};
    this.#dispatched = false;
    this.#invoke = invoke;
    this.#projectCollections = projectCollections;
    this.#supportsAction = supportsAction;
    this.#disabled = view.show_state !== 'available' || view.interactions_admitted !== true;

    this.#layoutObserver?.disconnect();
    const component = view.component;
    const expression = component?.expression?.expression;
    for (const key of ['componentExpression', 'componentDensity', 'componentPalette', 'componentFallback', 'componentColumns']) delete this.#element.dataset[key];
    for (const key of ['--face-columns', '--face-gutter', '--face-text-scale']) this.#element.style.removeProperty(key);
    if (component && expression) {
      this.#element.dataset.componentExpression = expression.identity;
      this.#element.dataset.componentDensity = expression.density;
      this.#element.dataset.componentPalette = expression.palette;
      this.#element.dataset.componentFallback = component.fallback ?? '';
      this.#element.style.setProperty('--face-text-scale', String(expression.text_scale_percent / 100));
      this.#element.style.setProperty('--face-gutter', `${component.gutter}px`);
      const reflow = () => {
        const width = this.#element.getBoundingClientRect().width;
        const available = Math.max(0, width - 2 * component.gutter);
        const columns = !component.fallback && available >= component.reflow_width ? component.maximum_columns : 1;
        this.#element.style.setProperty('--face-columns', String(columns));
        this.#element.dataset.componentColumns = String(columns);
      };
      reflow();
      if (globalThis.ResizeObserver) {
        this.#layoutObserver = new ResizeObserver(reflow);
        this.#layoutObserver.observe(this.#element);
      }
    }
    const bindRegion = (node, subject) => {
      if (!component) return;
      const subjectIndex = view.subjects.findIndex(candidate => candidate.identity === subject.identity);
      const region = component.subject_regions[subjectIndex];
      if (region !== null && region !== undefined) {
        node.dataset.componentRegion = String(region);
        node.dataset.componentGroup = String(component.groups[region]);
      } else node.dataset.componentFallback = 'unmatched';
    };
    const names = new Map(view.subjects.map(subject => [subject.identity, subject.name]));
    const documentNode = faceNode('article', '', 'owner-face-document');
    documentNode.setAttribute('part', 'document');
    const { collections, placed } = this.#projectCollections(view);
    const renderedActions = new Set();
    const actionReady = action => this.#ready(view, action, render);
    const actionControl = (action, itemName) => {
      renderedActions.add(action.identity);
      const control = document.createElement('form');
      control.setAttribute('part', 'action');
      control.dataset.ownerAction = action.identity;
      if (!itemName) control.append(faceNode('h5', action.name));
      const inputs = [];
      const composing = new Set();
      const argumentCount = Array.isArray(action.arguments) ? action.arguments.length : 0;
      const argumentNames = new Set();
      let supported = Array.isArray(action.arguments) && argumentCount <= 64
        && action.arguments.every(argument => {
          if (typeof argument?.name !== 'string' || !argument.name.trim() || argumentNames.has(argument.name)
            || typeof argument.value_name !== 'string' || !argument.value_name.trim()) return false;
          argumentNames.add(argument.name);
          return true;
        });
      for (const argument of supported ? action.arguments : []) {
        if (!argument || !Number.isSafeInteger(argument.maximum_bytes)
          || argument.maximum_bytes < 0 || argument.maximum_bytes > 4096) {
          supported = false;
          continue;
        }
        const label = faceNode('label', argument.value_name);
        let input;
        if (Array.isArray(argument.choices) && argument.choices.length) {
          if (argument.choices.length > 64 || argument.choices.some(choice =>
            typeof choice !== 'string' || choice === '' || !choice.isWellFormed() || encoder.encode(choice).length > argument.maximum_bytes)) {
            supported = false;
            continue;
          }
          const escapeChoices = argument.choices.some(choice => /^\s|\s$|\s{2}|[^\S ]|[\u0000-\u001f\u007f]/u.test(choice));
          input = document.createElement('select');
          const prompt = document.createElement('option');
          prompt.value = ''; prompt.textContent = 'Choose an option';
          input.append(prompt);
          for (const choice of argument.choices) {
            const option = document.createElement('option');
            option.value = choice; option.textContent = escapeChoices ? exactChoiceLabel(choice) : choice;
            input.append(option);
          }
          input.required = true;
        } else if (Array.isArray(argument.choices) && argument.value_kind === 'value/text') {
          input = document.createElement('input');
          input.type = 'text'; input.maxLength = argument.maximum_bytes;
          input.addEventListener('input', () => input.setCustomValidity(
            argumentValidity(input.value, argument.maximum_bytes)));
        } else {
          supported = false;
          continue;
        }
        input.addEventListener('compositionstart', () => composing.add(input));
        input.addEventListener('compositionend', () => composing.delete(input));
        label.setAttribute('part', 'label');
        input.setAttribute('part', 'input');
        input.dataset.ownerFocus = JSON.stringify([view.body_id, action.identity, action.target, 'argument', argument.name]);
        input.dataset.unavailable = String(action.availability !== 'available' || !this.#supportsAction(action));
        input.dataset.ownerDraft = JSON.stringify([view.body_id, action.identity, action.target, argument]);
        const draft = drafts.get(input.dataset.ownerDraft);
        if (draft !== undefined && input.dataset.unavailable !== 'true'
          && (argument.choices.length ? argument.choices.includes(draft)
            : encoder.encode(draft).length <= argument.maximum_bytes)) input.value = draft;
        label.append(input);
        control.append(label);
        inputs.push({ name: argument.name, input, argument });
      }
      const button = faceNode('button', itemName ? `${action.name} ${itemName}` : action.name);
      button.setAttribute('part', 'button');
      button.type = 'submit';
      button.dataset.ownerFocus = JSON.stringify([view.body_id, action.identity, action.target, 'submit']);
      button.disabled = !actionReady(action) || !supported || inputs.length !== argumentCount;
      button.dataset.unavailable = String(action.availability !== 'available' || !this.#supportsAction(action)
        || !supported || inputs.length !== argumentCount);
      control.append(button);
      if (button.disabled) control.append(faceNode('p', !this.#supportsAction(action)
        ? 'This action is unavailable in this view.' : action.explanation ??
        (supported ? 'This owner action is unavailable.' : 'This input has no supported browser form.')));
      control.addEventListener('submit', async event => {
        event.preventDefault();
        if (!this.#reachable(button) || !this.#ready(view, action, render) || !this.#invoke
          || !supported || inputs.length !== argumentCount || composing.size) return;
        // A submit event can be synthetic, and script can change values without
        // firing input events. Recheck the admitted argument domain here.
        for (const { input, argument } of inputs) {
          if (!this.#reachable(input)) return;
          if (argument.choices.length) {
            if (!argument.choices.includes(input.value)) return;
          }
          input.setCustomValidity(argumentValidity(input.value, argument.maximum_bytes));
          if (!input.checkValidity()) return;
        }
        this.#dispatched = true;
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
      bindRegion(node, subject);
      node.append(faceNode('span', subject.semantic_role ?? subject.role, 'owner-face-role'));
      node.append(faceNode('h4', subject.name));
      for (const text of subject.text) node.append(faceNode('p', text));
      if (subject.values?.length) {
        const values = document.createElement('dl');
        for (const value of subject.values) {
          const output = faceNode('dd', value.text, 'owner-face-value');
          output.setAttribute('part', 'value');
          values.append(faceNode('dt', value.name), output);
        }
        node.append(values);
      }
      const flags = Array.isArray(subject.flags) ? subject.flags.filter(flag =>
        typeof flag?.name === 'string' && typeof flag.value === 'boolean') : [];
      if (subject.properties.length || flags.length) {
        const details = localDetails(subject.identity, 'facts', 'Exact facts');
        const list = document.createElement('ul');
        for (const property of subject.properties) {
          const item = document.createElement('li'); item.textContent = property; list.append(item);
        }
        for (const flag of flags) list.append(faceNode('li', `${flag.name}: ${flag.value}`));
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
      bindRegion(section, collection);
      section.setAttribute('part', 'collection');
      section.append(faceNode('h4', collection.name));
      for (const text of collection.text) section.append(faceNode('p', text));
      section.append(faceNode('p', count, 'owner-face-collection-count'));
      const openList = document.createElement('ol');
      openList.className = 'owner-face-items';
      openList.setAttribute('aria-label', `Things left in ${collection.name}`);
      for (const item of open) {
        const row = document.createElement('li');
        bindRegion(row, item);
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
        const details = localDetails(collection.identity, 'completed',
          `Show ${completed.length} completed ${completed.length === 1 ? 'item' : 'items'}`);
        details.className = 'owner-face-completed';
        const completedList = document.createElement('ol');
        completedList.className = 'owner-face-items';
        completedList.setAttribute('aria-label', `Completed items in ${collection.name}`);
        for (const item of completed) {
          const row = document.createElement('li');
          bindRegion(row, item);
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
    const hasApplication = collections.length > 0 || view.subjects.some(subject =>
      !['Body', 'Plot'].includes(subject.role) && subject.disclosure === 'Primary');
    const bodyMenu = localDetails(null, 'body-menu', 'Body');
    bodyMenu.dataset.bodyMenu = '';
    const choiceGroups = [];
    const inspection = localDetails(null, 'inspection', 'Inspect other Face subjects');
    for (const group of view.subjects) {
      if (group.semantic_role !== 'conduit.face/choice-group@1'
        || group.choice_multiplicity !== 'exclusive' || group.values?.length || group.unmodeled_property_count !== 0
        || group.flags?.length) continue;
      const options = view.relationships
        .filter(relation => relation.kind === 'Contains' && relation.source === group.identity)
        .map(relation => view.subjects.find(subject => subject.identity === relation.target));
      // A radio group requires exact semantic selection flags and one offered
      // zero-input action per option. Unfamiliar contracts keep the generic view.
      const offered = options.map(option => view.actions.filter(action => action.target === option?.identity));
      const selectedFlags = options.map(option => Array.isArray(option?.flags)
        ? option.flags.filter(flag => flag?.name === 'selected') : []);
      if (!options.length || new Set(options.map(option => option?.identity)).size !== options.length
        || options.some((option, index) => !option || option.text?.length || option.values?.length
          || option.semantic_role === 'conduit.face/choice-group@1' || option.choice_multiplicity != null
          || option.unmodeled_property_count !== 0 || selectedFlags[index].length !== 1 || option.flags.length !== 1
          || typeof selectedFlags[index][0].value !== 'boolean' || offered[index].length !== 1
          || !Array.isArray(offered[index][0].arguments) || offered[index][0].arguments.length !== 0)
        || selectedFlags.filter(flags => flags[0].value).length > 1) continue;
      const fieldset = document.createElement('fieldset');
      fieldset.className = 'owner-face-choice-group';
      bindRegion(fieldset, group);
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
        input.dataset.ownerFocus = JSON.stringify([view.body_id, action.identity, action.target, 'choice']);
        input.name = group.identity;
        input.value = option.identity;
        input.checked = selectedFlags[index][0].value;
        input.disabled = !actionReady(action);
        input.dataset.unavailable = String(action.availability !== 'available' || !this.#supportsAction(action));
        label.dataset.ownerAction = action.identity;
        renderedActions.add(action.identity);
        label.append(input, faceNode('span', option.name));
        if (action.explanation) label.append(faceNode('small', action.explanation));
        input.addEventListener('change', async () => {
          if (!input.checked || !this.#reachable(input, label) || !this.#ready(view, action, render) || !this.#invoke) return;
          // Selection remains the acknowledged Face's fact until a fresh Show.
          choices.querySelectorAll('input').forEach((control, optionIndex) => {
            control.checked = selectedFlags[optionIndex][0].value;
          });
          this.#dispatched = true;
          this.disabled = true;
          await this.#invoke({ view, action, arguments: [] });
        });
        choices.append(label);
        placed.add(option.identity);
      });
      fieldset.append(choices);
      choiceGroups.push(fieldset);
      placed.add(group.identity);
    }
    for (const subject of view.subjects) {
      if (placed.has(subject.identity)) continue;
      const management = hasApplication
        && (['Body', 'Plot'].includes(subject.role) || subject.disclosure === 'Context');
      const main = ['Primary', 'Context'].includes(subject.disclosure)
        && (!collections.length || subject.role === 'Status');
      (management ? bodyMenu : main ? documentNode : inspection).append(renderSubject(subject));
    }
    documentNode.append(...choiceGroups);
    if (bodyMenu.children.length > 1) documentNode.append(bodyMenu);
    if (inspection.children.length > 1) documentNode.append(inspection);
    if (view.relationships.length) {
      const relations = localDetails(null, 'relationships', 'How these parts relate');
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
      const secondary = localDetails(null, 'secondary-actions', 'More actions and unavailable controls');
      for (const action of view.actions) {
        if (renderedActions.has(action.identity)) continue;
        const container = actionReady(action)
          && action.disclosure === 'CurrentAction' ? actions : secondary;
        container.append(actionControl(action));
      }
      if (actions.children.length > 1) documentNode.append(actions);
      if (secondary.children.length > 1) documentNode.append(secondary);
    }
    if (component && !component.fallback) {
      // Group only consecutive rendered roots. Source grouping never moves a
      // subject past another root or changes native list/control semantics.
      const roots = [...documentNode.children];
      let run, previousGroup;
      for (const node of roots) {
        const group = node.dataset.componentGroup;
        if (group === undefined) {
          run = undefined; previousGroup = undefined;
          continue;
        }
        if (!run || group !== previousGroup) {
          run = document.createElement('div');
          run.className = 'owner-component-region-run';
          run.dataset.componentGroup = group;
          documentNode.insertBefore(run, node);
          previousGroup = group;
        }
        run.append(node);
      }
    }
    this.#surface.replaceChildren(documentNode);
    this.#element.dataset.faceId = view.face_id;
    this.#element.dataset.faceRevision = view.face_revision;
    this.#element.dataset.showId = view.show_id;
    this.disabled = this.#disabled;
    if (focusedKey) {
      const current = [...this.#surface.querySelectorAll('[data-owner-focus]')]
        .find(control => control.dataset.ownerFocus === focusedKey);
      // Focus is local presentation state, never an action or permission.
      // A disabled control can lose focus to the document while a Show is
      // prepared. Carry its key, but never take focus from another control.
      const active = this.#element.ownerDocument.activeElement;
      const mayRestore = active === this.#element || active === this.#element.ownerDocument.body;
      this.#pendingFocus = undefined;
      if (mayRestore && current) {
        const visibleControl = current.matches('input[type=radio]') ? current.closest('label') : current;
        if (this.#reachable(current, visibleControl)) {
          current.focus();
          if (current.value === focusState.value && Number.isInteger(focusState.start)
            && Number.isInteger(focusState.end) && typeof current.setSelectionRange === 'function') {
            current.setSelectionRange(focusState.start, focusState.end, focusState.direction);
          }
        } else if (this.#disabled) this.#pendingFocus = focusState;
      }
    }
  }
}
