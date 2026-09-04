export const PRODUCT_DESTINATIONS = Object.freeze([
  Object.freeze({ identity: "home", key: "home", label: "conduit" }),
  Object.freeze({ identity: "tour", key: "tour", label: "Tour" }),
  Object.freeze({ identity: "creche", key: "creche", label: "Crèche" }),
  Object.freeze({ identity: "patchbay", key: "patchbay", label: "Patchbay" }),
  Object.freeze({ identity: "source", key: "source", label: "Source" }),
]);

const PRODUCTS = new Set(["home", "tour", "creche", "patchbay"]);
const STATUS_COMPONENTS = new Set(["status", "success-status", "warning-status", "failure-status"]);

export function productMastheadNodes({ parent = null, firstIndex = 0, current = null, status, statusComponent = "status" }) {
  if (!Number.isSafeInteger(firstIndex) || firstIndex < 0) throw new TypeError("masthead first index is invalid");
  if (parent !== null && (!Number.isSafeInteger(parent) || parent < 0 || parent >= firstIndex)) throw new TypeError("masthead parent is invalid");
  if (current !== null && !PRODUCTS.has(current)) throw new TypeError("masthead current product is not admitted");
  if (typeof status !== "string" || !STATUS_COMPONENTS.has(statusComponent)) throw new TypeError("masthead status is invalid");
  const navigationIndex = firstIndex + 1;
  return Object.freeze([
    Object.freeze({ parent, component: "masthead", key: "product-masthead", text: "", action: null }),
    Object.freeze({ parent: firstIndex, component: "navigation", key: "product-navigation", text: "Conduit products", value: current ?? "", valueCapacity: current === null ? 0 : 16, action: null }),
    ...PRODUCT_DESTINATIONS.map((destination) => Object.freeze({
      parent: navigationIndex,
      component: "navigation-link",
      key: destination.key,
      text: destination.label,
      value: destination.identity,
      valueCapacity: 16,
      action: null,
    })),
    Object.freeze({ parent: firstIndex, component: statusComponent, key: "product-status", text: status, action: null }),
  ]);
}

export function productMastheadDescription({ revision, current, status, statusComponent = "status" }) {
  if (!Number.isSafeInteger(revision) || revision < 1) throw new TypeError("masthead revision must be positive");
  return Object.freeze({
    revision,
    actions: Object.freeze([]),
    nodes: productMastheadNodes({ current, status, statusComponent }),
  });
}

export function createProductMasthead(presentation, slot, current) {
  let revision = 0;
  const show = (status, statusComponent) => presentation.present(slot, productMastheadDescription({
    revision: ++revision,
    current,
    status,
    statusComponent,
  }));
  return Object.freeze({
    ordinary: (status) => show(status, "status"),
    success: (status) => show(status, "success-status"),
    warning: (status) => show(status, "warning-status"),
    failure: (status) => show(status, "failure-status"),
    present(status, statusComponent = "status") { return show(status, statusComponent); },
  });
}
