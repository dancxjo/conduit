// Native text editing with a finite manual-selection popup. No source submission.
export function attachPortableSuggestions(input, container, { options, policy, currentBytes, id, onDraft }) {
  if (options.length > 64) throw new Error('Suggestion capacity');
  const list = document.createElement('ul');
  list.id = `${id}-suggestions`; list.setAttribute('role', 'listbox'); list.hidden = true;
  list.setAttribute('aria-label', `${input.getAttribute('aria-label')} suggestions`);
  input.setAttribute('aria-haspopup', 'listbox'); input.setAttribute('role', 'combobox'); input.setAttribute('aria-autocomplete', 'list');
  input.setAttribute('aria-controls', list.id); input.setAttribute('aria-expanded', 'false');
  const same = (a, b) => a.length === b.length && a.every((byte, index) => byte === b[index]);
  const initial = options.find(option => same(option.bytes, currentBytes));
  let selected = policy === 'fixed' ? initial?.identity ?? null : null;
  let visible = [], active = -1, composing = false;
  const close = () => {
    list.hidden = true; input.setAttribute('aria-expanded', 'false');
    input.removeAttribute('aria-activedescendant'); active = -1;
  };
  const highlight = index => {
    active = index;
    for (const [position, item] of [...list.children].entries()) item.setAttribute('aria-selected', String(position === active));
    if (active >= 0) {
      input.setAttribute('aria-activedescendant', list.children[active].id);
      list.children[active].scrollIntoView({ block: 'nearest' });
    } else input.removeAttribute('aria-activedescendant');
  };
  const choose = option => {
    selected = option.identity;
    input.value = policy === 'fixed' ? option.name : option.text;
    onDraft(); close(); input.focus();
  };
  const open = (filter = true) => {
    if (composing || input.disabled) return;
    const query = input.value.toLocaleLowerCase();
    visible = options.filter(option => !filter || option.name.toLocaleLowerCase().includes(query) || option.text.toLocaleLowerCase().includes(query));
    list.replaceChildren(); active = -1;
    for (const [index, option] of visible.entries()) {
      const item = document.createElement('li'); item.id = `${list.id}-${index}`;
      item.setAttribute('role', 'option'); item.setAttribute('aria-selected', 'false');
      item.textContent = option.name; item.dataset.sourceOption = option.identity; item.dataset.sourceValue = option.text;
      item.addEventListener('pointerdown', event => { if (event.isTrusted) event.preventDefault(); });
      item.addEventListener('click', event => { if (event.isTrusted && !input.disabled && !composing) choose(option); });
      list.append(item);
    }
    // Keep popup dismissal from moving a control between pointerdown and click.
    list.style.top = `${input.offsetTop + input.offsetHeight}px`;
    list.hidden = !visible.length; input.setAttribute('aria-expanded', String(visible.length > 0));
    input.removeAttribute('aria-activedescendant');
  };
  input.value = policy === 'fixed' && initial ? initial.name : input.value;
  input.addEventListener('input', event => {
    if (!event.isTrusted) return;
    selected = null; onDraft(); if (!composing) open();
  });
  input.addEventListener('compositionstart', event => { if (!event.isTrusted) return; composing = true; close(); });
  input.addEventListener('compositionend', event => { if (!event.isTrusted) return; composing = false; selected = null; });
  input.addEventListener('blur', close);
  input.addEventListener('keydown', event => {
    if (!event.isTrusted || event.isComposing || composing || event.ctrlKey || event.metaKey || event.altKey) return;
    if (!event.shiftKey && (event.key === 'ArrowDown' || event.key === 'ArrowUp')) {
      event.preventDefault(); if (list.hidden) open(false);
      if (visible.length) highlight(Math.max(0, Math.min(visible.length - 1, active < 0 ? (event.key === 'ArrowDown' ? 0 : visible.length - 1) : active + (event.key === 'ArrowDown' ? 1 : -1))));
    } else if (event.key === 'Escape' && !list.hidden) { event.preventDefault(); close(); }
    else if (event.key === 'Enter' && !list.hidden && active >= 0) { event.preventDefault(); event.stopPropagation(); choose(visible[active]); }
  });
  container.append(list);
  return {
    bytes(model, maximum) {
      if (composing) throw new Error('Text composition is pending');
      if (policy === 'fixed') {
        const option = options.find(option => option.identity === selected);
        if (!option) throw new Error('Choose a declared suggestion before submitting');
        return option.bytes;
      }
      return model.textBytes(input.value, maximum);
    },
    reset() { selected = policy === 'fixed' ? initial?.identity ?? null : null; input.value = policy === 'fixed' && initial ? initial.name : new TextDecoder().decode(Uint8Array.from(currentBytes)); close(); },
    composing: () => composing,
    close,
  };
}
