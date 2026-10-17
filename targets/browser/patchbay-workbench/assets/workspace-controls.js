import {
  annotateFlow, collapseFlow, exportFlowWorkspace, flowSceneSnapshot, flowWorkspaceStatus,
  frameFlow, importFlowWorkspace, retainedFlowWorkspaces, routeFlow, saveFlowLayout, selectFlowLayout,
} from "/assets/flow.js";

export function unusedAnnotationId(items, prefix) {
  const existing = new Set(items.map(item => item.id));
  for (let index = 1; index <= items.length + 1; index += 1) {
    const id = `${prefix}-${index}`;
    if (!existing.has(id)) return id;
  }
}

export function installWorkspaceControls(root, selected, restoreLens, navigationVersion = () => 0) {
  const pending = new Set();
  const status = document.createElement("output");
  status.id = "workspace-status";
  status.setAttribute("role", "status");
  function field(labelText, control) {
    const label = document.createElement("label");
    label.textContent = labelText;
    control.setAttribute("aria-label", labelText);
    label.append(control);
    root.append(label);
    return control;
  }
  const name = field("Layout name", document.createElement("input"));
  name.value = "Default";
  name.maxLength = 64;
  const layouts = field("Saved layouts", document.createElement("select"));
  const text = field("Workspace annotation", document.createElement("input"));
  text.maxLength = 1024;
  const route = field("Visual Cord routing", document.createElement("select"));
  for (const style of ["straight", "curved", "orthogonal", "manual"]) {
    const option = document.createElement("option");
    option.value = option.textContent = style;
    route.append(option);
  }
  const points = field("Manual route points (x,y; x,y)", document.createElement("input"));
  points.maxLength = 1024;
  const encoded = field("Workspace document", document.createElement("textarea"));
  encoded.maxLength = 64 * 1024;
  const retained = field("Retained workspaces", document.createElement("select"));

  function update(value = flowWorkspaceStatus()) {
    layouts.replaceChildren(...value.layouts.map(layout => {
      const option = document.createElement("option");
      option.value = option.textContent = layout;
      return option;
    }));
    layouts.value = value.active_layout;
    status.textContent = `${value.status}${value.notice ? ` · ${JSON.stringify(value.notice)}` : ""} · ${value.correlation?.orphaned_subjects.length ?? 0} orphaned subjects`;
    if (value.retained_workspaces?.length) status.textContent += ` · Retained correlation: ${JSON.stringify(value.retained_workspaces)}`;
    for (const control of root.querySelectorAll('button[data-needs-workspace="true"]')) {
      control.disabled = pending.has(control) || value.available === false;
    }
  }
  function button(label, action, needsWorkspace = true) {
    const control = document.createElement("button");
    control.type = "button";
    control.textContent = label;
    control.dataset.needsWorkspace = String(needsWorkspace);
    control.onclick = async () => {
      pending.add(control);
      control.disabled = true;
      try { await action(); update(); }
      catch (error) { status.textContent = `Workspace refused: ${error.message}${error.correlation ? ` · ${JSON.stringify(error.correlation)}` : ""}`; }
      finally {
        pending.delete(control);
        control.disabled = needsWorkspace && flowWorkspaceStatus().available === false;
      }
    };
    root.append(control);
  }
  button("Save layout", () => saveFlowLayout(name.value));
  button("Use layout", async () => {
    const version = navigationVersion();
    const result = await selectFlowLayout(layouts.value);
    if (navigationVersion() === version) await restoreLens(result.lens);
  });
  button("Add note", () => {
    const scene = flowSceneSnapshot();
    const node = scene?.nodes.find(node => node.id === selected());
    const notes = JSON.parse(exportFlowWorkspace()).layouts.find(layout => layout.name === flowWorkspaceStatus().active_layout).notes;
    return annotateFlow({ id: unusedAnnotationId(notes, "note"), text: text.value, subject: node?.data.workspaceSubject ?? null, x: 80, y: 40 });
  });
  button("Frame selected Gears", () => {
    const scene = flowSceneSnapshot();
    const members = scene?.nodes.filter(node => node.selected || node.id === selected()).map(node => node.data.workspaceSubject).filter(Boolean) ?? [];
    if (!members.length) throw new Error("Select Gear subjects first");
    const frames = JSON.parse(exportFlowWorkspace()).layouts.find(layout => layout.name === flowWorkspaceStatus().active_layout).frames;
    return frameFlow({ id: unusedAnnotationId(frames, "frame"), title: text.value || "Visual frame", members, x: 40, y: 80, width: 640, height: 320, collapsed: false });
  });
  button("Collapse selected visually", () => collapseFlow(selected(), true));
  button("Expand selected visually", () => collapseFlow(selected(), false));
  button("Apply visual Cord route", () => {
    const bends = points.value.trim() ? points.value.split(";").map(pair => {
      const coordinates = pair.split(",").map(value => Number(value.trim()));
      if (coordinates.length !== 2 || !coordinates.every(Number.isInteger)) throw new Error("Route points must be integer x,y pairs");
      return { x: coordinates[0], y: coordinates[1] };
    }) : [];
    return routeFlow(selected(), route.value, bends);
  });
  button("Export workspace", () => { encoded.value = exportFlowWorkspace(); });
  button("Import workspace", async () => {
    const version = navigationVersion();
    const result = await importFlowWorkspace(encoded.value);
    if (navigationVersion() === version) await restoreLens(result.lens);
  });
  button("Inspect retained workspaces", async () => {
    const documents = await retainedFlowWorkspaces();
    retained.replaceChildren(...documents.map(document => {
      const option = window.document.createElement("option");
      option.value = document.identity;
      option.textContent = `${document.basis.checked_plot_id} · ${document.correlation.orphaned_subjects.length} orphans`;
      return option;
    }));
  }, false);
  button("Export retained workspace", () => {
    if (!retained.value) throw new Error("Inspect and select a retained workspace first");
    encoded.value = exportFlowWorkspace(retained.value);
  }, false);
  root.append(status);
  update();
  return update;
}
