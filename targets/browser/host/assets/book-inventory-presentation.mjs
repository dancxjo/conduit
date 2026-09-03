let revision = 0;

export function presentBookInventory(presentation, inventory) {
  const installed = inventory.entries.filter((entry) => entry.implementation_id !== null);
  const nodes = [
    {
      parent: null,
      component: "disclosure",
      key: "inventory",
      text: `Available gears · ${installed.length} exact browser implementations · ${inventory.limits.maximum_gears} Gear / ${inventory.limits.maximum_cords} Cord bound`,
      action: null,
    },
    { parent: 0, component: "definition-table", key: "offers", text: "Exact browser planning offers", action: null },
  ];
  for (const [index, entry] of inventory.entries.entries()) {
    const availability = entry.implementation_id ? "available" : "unavailable";
    nodes.push({
      parent: 1,
      component: "definition",
      key: `offer-${availability}-${index}`,
      text: entry.kind_id,
      value: `${entry.family} · ${entry.classification} · ${entry.reason}`,
      valueCapacity: 1024,
      action: null,
    });
  }
  presentation.present("book-inventory", { revision: ++revision, actions: [], nodes });
}
