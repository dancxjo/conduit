const DOCKS = ["left", "right", "bottom"];

function button(label, action) {
  const control = document.createElement("button");
  control.type = "button";
  control.textContent = label;
  control.dataset.furnitureAction = action;
  return control;
}

export function installPanelFurniture(entries) {
  const panels = new Map();
  for (const entry of entries) {
    const surface = document.querySelector(entry.selector);
    if (!surface) throw new Error(`missing furniture surface ${entry.selector}`);
    const bar = document.createElement("div"), title = document.createElement("strong");
    const collapse = button(`Collapse ${entry.title}`, "collapse");
    const move = button("", "move");
    const close = button(`Close ${entry.title}`, "close");
    bar.className = "furniture-bar";
    bar.setAttribute("role", "toolbar");
    bar.setAttribute("aria-label", `${entry.title} furniture`);
    title.textContent = entry.title;
    bar.append(title, collapse, move, close);
    surface.prepend(bar);
    surface.dataset.furnitureSurface = entry.name;
    surface.dataset.furnitureDock = entry.dock;
    surface.dataset.furnitureCollapsed = "false";
    const update = () => {
      const collapsed = surface.dataset.furnitureCollapsed === "true";
      const dock = surface.dataset.furnitureDock;
      collapse.textContent = `${collapsed ? "Expand" : "Collapse"} ${entry.title}`;
      collapse.setAttribute("aria-expanded", String(!collapsed));
      move.textContent = `Move ${entry.title} to ${DOCKS[(DOCKS.indexOf(dock) + 1) % DOCKS.length]}`;
    };
    collapse.onclick = () => { surface.dataset.furnitureCollapsed = String(surface.dataset.furnitureCollapsed !== "true"); update(); };
    move.onclick = () => { surface.dataset.furnitureDock = DOCKS[(DOCKS.indexOf(surface.dataset.furnitureDock) + 1) % DOCKS.length]; update(); };
    close.onclick = () => entry.onDismiss();
    panels.set(entry.name, { surface, collapse, update });
    update();
  }
  return {
    restore(name) {
      const panel = panels.get(name);
      if (!panel) throw new Error(`unknown furniture surface ${name}`);
      panel.surface.dataset.furnitureCollapsed = "false";
      panel.update();
    },
  };
}
