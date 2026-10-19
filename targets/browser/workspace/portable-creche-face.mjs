import { renderPortableFace } from "../../../targets/browser/host/assets/portable-face-controls.mjs";

const encoder = new TextEncoder(), decoder = new TextDecoder('utf-8', { fatal: true });

// A local Host carrier for the actual Crèche owner. It holds one current Show;
// all values and action contracts are read from that owner's canonical Face.
export function createPortableCrecheFace({ root, host, onSnapshot, onRefusal }) {
  const mount = root;
  const toolbar = document.createElement('div');
  root = document.createElement('div'); mount.replaceChildren(toolbar, root);
  const refresh = document.createElement('button'); refresh.type = 'button'; refresh.textContent = 'Refresh controls'; refresh.hidden = true;
  toolbar.append(refresh);
  const runtime = host.runtime;
  let current = null, controller = null, sequence = 0, generation = null;
  let admission = null, retired = false, presenting = false, preferredComponent = true, latestSnapshot = null;
  const output = () => JSON.parse(decoder.decode(new Uint8Array(runtime.memory.buffer,
    runtime.conduit_creche_local_face_output_ptr(), runtime.conduit_creche_local_face_output_len())));
  const call = (name, request) => {
    const bytes = encoder.encode(JSON.stringify(request));
    if (bytes.length > runtime.conduit_creche_input_capacity()) throw new Error('Crèche Host request capacity');
    new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_input_ptr(), bytes.length).set(bytes);
    const code = runtime[`conduit_creche_local_face_${name}`](bytes.length);
    const response = output();
    if (code < 0) throw new Error(response.message ?? `Crèche Host refused (${code})`);
    return response;
  };
  const close = show => {
    if (!current || current.view.show_id !== show) return;
    controller?.retire(); current = null;
    runtime.conduit_creche_local_face_close(generation);
  };
  async function prepare(snapshot) {
    latestSnapshot = snapshot; generation = snapshot.generation;
    if (admission === null) {
      if (runtime.conduit_creche_local_face_component_descriptor() < 0) throw new Error('Component descriptor refused');
      const descriptor = output();
      admission = typeof host.admitPortableFaceComponent === 'function'
        ? await host.admitPortableFaceComponent({ descriptor, render: renderPortableFace })
        : { selected: false, reason: 'This Host has no optional component loader.' };
    }
    if (retired || !root.isConnected) return;
    if (admission.selected && !toolbar.querySelector("select")) {
      const label = document.createElement('label'); label.textContent = 'Controls on this Host ';
      const choice = document.createElement('select'); choice.setAttribute('aria-label', 'Controls on this Host');
      for (const [value, text] of [['enhanced', 'Enhanced controls'], ['basic', 'Basic controls']]) {
        const option = document.createElement('option'); option.value = value; option.textContent = text; choice.append(option);
      }
      choice.addEventListener('change', event => {
        if (!event.isTrusted || retired || presenting) return;
        preferredComponent = choice.value === 'enhanced';
        void carrier.present(latestSnapshot);
      });
      label.append(choice); toolbar.append(label);
    }
    const selected = admission.selected && preferredComponent;
    const response = call('prepare', { generation, host_id: host.hostId, boot_id: host.bootId,
      component: selected, component_admission: selected
        ? { identity: admission.identity, version: 1, tag: "conduit-portable-face", bundle_sha256: admission.bundle_sha256 } : null });
    const encounter = response.encounter;
    controller?.retire(); current = encounter;
    const callbacks = {
      onRefusal: report,
      disconnected: close,
      async submit({ action, arguments: args, view }) {
        if (retired || view.show_id !== current?.view.show_id || !root.isConnected || document.visibilityState !== 'visible') throw new Error('Crèche Show replaced or hidden');
        if (++sequence > 0xffff_ffff) throw new Error('Crèche Host sequence exhausted');
        const result = call('interact', { generation, interaction: {
          show_id: view.show_id, face_id: view.face_id, face_revision: view.face_revision,
          action_id: action.identity, target: action.target, arguments: args, sequence,
        } });
        close(view.show_id);
        // The old encounter is a consumed receipt. Only the owner snapshot may
        // update lifecycle/review state, then a fresh Show is prepared.
        onSnapshot(result.source_snapshot);
      },
    };
    try {
    if (selected) {
      const element = new admission.constructor();
      root.replaceChildren(element);
      controller = element.present(encounter, callbacks);
      element.inspect();
    } else {
      const generic = document.createElement('section'); generic.dataset.portableFaceBack = 'generic';
      const reason = document.createElement('p'); reason.dataset.componentRefusal = 'true'; reason.textContent = admission.selected ? 'Basic controls selected on this Host.' : admission.reason;
      const surface = document.createElement('div'); generic.append(reason, surface); root.replaceChildren(generic);
      controller = renderPortableFace(surface, encounter, callbacks); controller.inspect();
    }
    } catch (error) {
      if (!selected) throw error;
      close(encounter.view.show_id);
      admission = { selected: false, reason: `Component surface refused: ${error.message}` };
      return prepare(snapshot);
    }
    mount.dataset.portableMaskPlan = encounter.view.mask_plan_id;
    mount.dataset.portableFaceId = encounter.face.identity;
    mount.dataset.portableFaceRevision = String(encounter.face.revision);
    const acknowledged = call('ack', { generation, show_id: encounter.view.show_id,
      face_id: encounter.view.face_id, face_revision: encounter.view.face_revision });
    current = acknowledged.encounter;
    controller.activate(acknowledged.encounter);
    refresh.hidden = true;
    if (response.source_snapshot.revision !== snapshot.revision) onSnapshot(response.source_snapshot);
  }
  const report = error => { refresh.hidden = false; onRefusal(error); };
  const carrier = {
    async present(snapshot) {
      if (retired || presenting) return;
      presenting = true;
      try { await Promise.resolve(); await prepare(snapshot); }
      catch (error) { controller?.retire(); if (current) close(current.view.show_id); report(error); }
      finally { presenting = false; }
    },
    retire() {
      retired = true; if (current) close(current.view.show_id);
      disconnect.disconnect(); document.removeEventListener('visibilitychange', visibility);
    },
  };
  const disconnect = new MutationObserver(() => { if (!mount.isConnected) carrier.retire(); });
  disconnect.observe(document.documentElement, { childList: true, subtree: true });
  const visibility = () => {
    if (document.visibilityState !== 'visible' && current) {
      close(current.view.show_id); refresh.hidden = false;
    }
  };
  document.addEventListener('visibilitychange', visibility);
  refresh.addEventListener('click', event => {
    if (event.isTrusted && !retired && !presenting && latestSnapshot) void carrier.present(latestSnapshot);
  });
  return carrier;
}
