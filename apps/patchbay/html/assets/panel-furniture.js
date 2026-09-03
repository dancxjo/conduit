const DOCKS = ["left", "right", "bottom"];

export function installPanelFurniture(entries, presentation) {
  if (!presentation) throw new TypeError("panel furniture requires the shared application presentation host");
  const panels = new Map();
  for (const entry of entries) {
    const surface = document.querySelector(entry.selector);
    if (!surface) throw new Error(`missing furniture surface ${entry.selector}`);
    surface.dataset.furnitureSurface = entry.name;
    surface.dataset.furnitureDock = entry.dock;
    surface.dataset.furnitureCollapsed = "false";
    let revision = 0;
    const present = () => {
      const collapsed = surface.dataset.furnitureCollapsed === "true";
      const dock = surface.dataset.furnitureDock;
      presentation.present(entry.slot, {
        revision: ++revision,
        actions: ["collapse", "move", "close"].map(id => ({ id, event: "activate" })),
        nodes: [
          { parent: null, component: "action-group", key: "furniture", text: `${entry.title} furniture`, action: null },
          { parent: 0, component: "paragraph", key: "title", text: entry.title, action: null },
          { parent: 0, component: "button", key: "collapse", text: `${collapsed ? "Expand" : "Collapse"} ${entry.title}`, action: 0 },
          { parent: 0, component: "button", key: "move", text: `Move ${entry.title} to ${DOCKS[(DOCKS.indexOf(dock) + 1) % DOCKS.length]}`, action: 1 },
          { parent: 0, component: "button", key: "close", text: `Close ${entry.title}`, action: 2 },
        ],
      }, { onEvent(event) {
        if (event.action === "collapse") surface.dataset.furnitureCollapsed = String(!collapsed);
        else if (event.action === "move") surface.dataset.furnitureDock = DOCKS[(DOCKS.indexOf(dock) + 1) % DOCKS.length];
        else if (event.action === "close") { entry.onDismiss(); return; }
        present();
      } });
    };
    panels.set(entry.name, { surface, present });
    present();
  }
  return {
    restore(name) {
      const panel = panels.get(name);
      if (!panel) throw new Error(`unknown furniture surface ${name}`);
      panel.surface.dataset.furnitureCollapsed = "false";
      panel.present();
    },
  };
}
