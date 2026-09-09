// Host realization of the shared portable Form library presentation.
const decoder = new TextDecoder('utf-8', { fatal: true });

export function openWorkspaceLibrary({ panel, session, source, inventory, presentationFor, onUse, onRemove, onClose, onFailure }) {
  const presentation = presentationFor(panel);
  let revision = 0, query = '', busy = false;
  const heading = panel.querySelector('h2');
  const render = () => {
    if (panel.hidden) return;
    if (revision === 0xffff_ffff) throw new Error('Form chooser presentation revision exhausted');
    const currentRevision = ++revision;
    const workloadRevision = session.current().workload_revision;
    const view = session.libraryView(source, query, currentRevision);
    presentation.present('workspace-form-library', view, { onEvent(event) {
      presentation.nextEvent('workspace-form-library');
      if (busy) return;
      try {
        if (event.revision !== revision) throw new Error('This Form chooser is stale');
        if (event.action === 'library.search' && event.kind === 3) {
          query = decoder.decode(event.value);
          render();
          return;
        }
        const match = /^library\.(use|remove)\.(\d+)$/u.exec(event.action);
        const form = match ? inventory.forms[Number(match[2])] : null;
        if (!form || event.kind !== 1 || event.value.length !== 0) throw new Error('Unknown Form chooser action');
        busy = true;
        panel.inert = true;
        const operation = match[1] === 'use' ? onUse : onRemove;
        Promise.resolve(operation(form, workloadRevision)).catch(onFailure).finally(() => {
          busy = false;
          panel.inert = false;
          try { render(); } catch (error) { onFailure(error); }
        });
      } catch (error) { onFailure(error); }
    } });
  };
  panel.querySelector('[data-close-library]').addEventListener('click', () => {
    if (!busy) onClose();
  });
  return Object.freeze({
    show() { panel.hidden = false; render(); heading.focus(); },
    hide() { panel.hidden = true; },
    isOpen: () => !panel.hidden,
    render,
  });
}
