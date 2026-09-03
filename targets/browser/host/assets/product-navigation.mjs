const DESTINATIONS = Object.freeze([
  Object.freeze({ id: "book", label: "Book", href: "/conduit/book/" }),
  Object.freeze({ id: "creche", label: "Crèche", href: "/conduit/creche/" }),
  Object.freeze({ id: "patchbay", label: "Patchbay", href: "/conduit/patchbay/" }),
  Object.freeze({ id: "source", label: "Source", href: "https://github.com/dancxjo/conduit" }),
]);

export function productDestinations() {
  return DESTINATIONS;
}

export function mountProductNavigation(scope = document) {
  const slots = [...scope.querySelectorAll("[data-conduit-product-navigation]")];
  for (const slot of slots) {
    const current = slot.dataset.currentProduct ?? "";
    if (current && !DESTINATIONS.some(({ id }) => id === current)) {
      throw new Error("shared product navigation has an unknown current destination");
    }
    slot.replaceChildren(...DESTINATIONS.map(({ id, label, href }) => {
      const link = document.createElement("a");
      link.href = href;
      link.textContent = label;
      link.dataset.productDestination = id;
      if (id === current) link.setAttribute("aria-current", "page");
      return link;
    }));
  }
  return Object.freeze({ destinations: DESTINATIONS, mounted: slots.length });
}
