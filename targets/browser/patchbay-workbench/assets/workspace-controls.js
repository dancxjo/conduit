import {
  annotateFlow, collapseFlow, exportFlowWorkspace, flowSceneSnapshot, flowWorkspaceStatus,
  frameFlow, importFlowWorkspace, routeFlow, saveFlowLayout, selectFlowLayout,
} from "/assets/flow.js";

export function installWorkspaceControls(root, selected, restoreLens) {
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
  encoded.maxLength = 256 * 1024;

  function update(value = flowWorkspaceStatus()) {
    layouts.replaceChildren(...value.layouts.map(layout => {
      const option = document.createElement("option");
      option.value = option.textContent = layout;
      return option;
    }));
    layouts.value = value.active_layout;
    status.textContent = `${value.status}${value.notice ? ` · ${JSON.stringify(value.notice)}` : ""} · ${value.correlation?.orphaned_subjects.length ?? 0} orphaned subjects`;
  }
  function button(label, action) {
    const control = document.createElement("button");
    control.type = "button";
    control.textContent = label;
    control.onclick = async () => {
      control.disabled = true;
      try { await action(); update(); }
      catch (error) { status.textContent = `Workspace refused: ${error.message}${error.correlation ? ` · ${JSON.stringify(error.correlation)}` : ""}`; }
      finally { control.disabled = false; }
    };
    root.append(control);
  }
  button("Save layout", () => saveFlowLayout(name.value));
  button("Use layout", async () => {
    const result = await selectFlowLayout(layouts.value);
    await restoreLens(result.lens);
  });
  button("Add note", () => {
    const scene = flowSceneSnapshot();
    const node = scene?.nodes.find(node => node.id === selected());
    const count = JSON.parse(exportFlowWorkspace()).layouts.find(layout => layout.name === flowWorkspaceStatus().active_layout).notes.length;
    return annotateFlow({ id: `note-${count + 1}`, text: text.value, subject: node?.data.workspaceSubject ?? null, x: 80, y: 40 });
  });
  button("Frame selected Gears", () => {
    const scene = flowSceneSnapshot();
    const members = scene?.nodes.filter(node => node.selected || node.id === selected()).map(node => node.data.workspaceSubject).filter(Boolean) ?? [];
    if (!members.length) throw new Error("Select Gear subjects first");
    const count = JSON.parse(exportFlowWorkspace()).layouts.find(layout => layout.name === flowWorkspaceStatus().active_layout).frames.length;
    return frameFlow({ id: `frame-${count + 1}`, title: text.value || "Visual frame", members, x: 40, y: 80, width: 640, height: 320, collapsed: false });
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
    const result = await importFlowWorkspace(encoded.value);
    await restoreLens(result.lens);
  });
  root.append(status);
  update();
  return update;
}
