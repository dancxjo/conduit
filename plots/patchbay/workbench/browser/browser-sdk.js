/**
 * Realize one immutable `@conduit/browser` Body Patchbay projection as a small
 * inspection Mask. Runtime and semantic truth stay in the SDK value; this
 * module owns only browser presentation mechanics.
 */
export function renderBrowserBodyPatchbay(root, snapshot) {
  if (typeof Element === "undefined" || !(root instanceof Element || root instanceof ShadowRoot)) {
    throw new TypeError("Patchbay workbench root must be one Element or ShadowRoot");
  }
  if (snapshot?.schema !== "conduit.browser/body-patchbay@1"
    || snapshot.topology?.schema !== "conduit.patchbay/checked-plot-projection@1"
    || !Array.isArray(snapshot.topology.gears)
    || !Array.isArray(snapshot.topology.cords)) {
    throw new TypeError("Patchbay workbench requires one bounded @conduit/browser Body projection");
  }

  const document = root.ownerDocument ?? root.host?.ownerDocument;
  const surface = document.createElement("section");
  surface.setAttribute("aria-label", "Patchbay checked Plot topology");
  surface.dataset.patchbaySchema = snapshot.schema;
  surface.dataset.bodyId = snapshot.bodyId;
  surface.dataset.bootId = snapshot.bootId;
  if (snapshot.planId) surface.dataset.planId = snapshot.planId;
  if (snapshot.playId) surface.dataset.playId = snapshot.playId;

  const heading = document.createElement("h2");
  heading.textContent = `Patchbay — ${snapshot.topology.plot_name}`;
  surface.append(heading);

  const gears = document.createElement("ol");
  gears.setAttribute("aria-label", "Gears");
  for (const gear of snapshot.topology.gears) {
    const item = document.createElement("li");
    item.dataset.patchbayGear = gear.gear_id;
    item.textContent = `${gear.gear_id} — ${gear.kind_id}`;
    gears.append(item);
  }
  surface.append(gears);

  const cords = document.createElement("ol");
  cords.setAttribute("aria-label", "Cords");
  for (const cord of snapshot.topology.cords) {
    const item = document.createElement("li");
    item.dataset.patchbayCord = `${cord.source_gear_id}.${cord.source_port_id}->${cord.sink_gear_id}.${cord.sink_port_id}`;
    item.textContent = `${cord.source_gear_id}.${cord.source_port_id} → ${cord.sink_gear_id}.${cord.sink_port_id} (${cord.info_kind}, ${cord.temporal})`;
    cords.append(item);
  }
  surface.append(cords);
  root.replaceChildren(surface);
}
