import { decorate, iconForRole, iconForToken } from "./gui-icons.mjs";

const React = window.React;
const Flow = window.ReactFlow;
const e = React.createElement;

// React owns the empty decoration slot; the shared finite renderer owns its SVG.
// Unknown authored tokens keep their original text instead of guessing a symbol.
function SemanticIcon({ role, token, fallback = "", title, className = "faceplate-icon" }) {
  const key = token ? iconForToken(token) : iconForRole(role);
  const mount = React.useCallback(node => {
    if (!node) return;
    node.replaceChildren();
    if (key) decorate(node, key);
    else node.textContent = fallback;
  }, [key, fallback]);
  return e("span", { ref: mount, className, title, "aria-hidden": "true" });
}

function PortRow({ port, onActivate }) {
  const receiving = port.direction === "receiving";
  // A Plot input supplies the interior graph; a Plot output receives from it.
  // This is Mask-local handle orientation, never a change to the Port contract.
  const target = port.boundary ? !receiving : receiving;
  const activity = port.debugger;
  const compatibility = port.compatibility
    ? (port.compatibility.compatible ? "compatible" : "incompatible")
    : "unknown";
  const diagnostic = port.compatibility?.diagnostic || undefined;
  const handle = e(Flow.Handle, {
    id: port.id,
    type: target ? "target" : "source",
    position: target ? Flow.Position.Left : Flow.Position.Right,
    isConnectable: true,
    className: "faceplate-handle",
    role: "img",
    "aria-label": port.boundary ? `Plot boundary: ${port.accessibilityName}` : port.accessibilityName,
    "data-port-id": port.id,
    "data-port-boundary": String(port.boundary === true),
    "data-port-direction": port.direction,
    "data-compatibility": compatibility,
    title: diagnostic,
  });
  return e("div", { className: `faceplate-port ${port.direction}${port.diagnosticError ? " diagnostic-error" : ""}${activity ? ` debugger-${activity.phase}` : ""}`, "data-port-id": port.id, "data-compatibility": compatibility, title: diagnostic, "data-debugger-phase": activity?.phase || "inactive", onClick:event=>{event.stopPropagation();if(!event.target.closest?.(".faceplate-handle"))onActivate(port.id);} },
    target && handle,
    e("button", {
      type: "button",
      className: "faceplate-port-name faceplate-port-select nodrag nowheel",
      title: port.accessibilityName,
      "aria-pressed": port.semanticSelected === true,
      "aria-label": `Select ${port.boundary ? "Plot boundary: " : ""}${port.accessibilityName}`,
      onClick: event => { event.stopPropagation(); onActivate(port.id); },
    }, e(SemanticIcon, { role: "Port", className: "faceplate-inline-icon" }), port.label),
    e("code", { title: port.valueKind }, port.valueKind),
    activity?.latest_value && e("output", { className: "debugger-value", "aria-label": `Latest observed value ${activity.latest_value.summary}` }, activity.latest_value.summary),
    !target && handle,
  );
}

export function FaceplateNode({ data }) {
  const title = data.title ?? data.label;
  return e("article", {
    className: `flow-frontplate role-${data.role.toLowerCase()}${data.workspaceCollapsed ? " workspace-collapsed" : ""}${data.semanticSelected ? " semantic-selected" : ""}${data.diagnosticError ? " diagnostic-error" : ""}${data.debugger ? ` debugger-${data.debugger.phase}` : ""}`,
    "data-subject": data.subjectIdentity,
    "data-subject-id": data.subjectIdentity,
    "data-lens": data.lens,
    "aria-label": data.accessibilityName,
    "data-debugger-phase": data.debugger?.phase || "inactive",
  },
  e("header", null,
    e("input", {
      type: "radio",
      className: "faceplate-selection nodrag nowheel",
      name: data.selectionGroup,
      value: data.subjectIdentity,
      checked: data.semanticSelected,
      "aria-label": `Select ${data.accessibilityName}`,
      "data-subject": data.subjectIdentity,
      onClick: (event) => event.stopPropagation(),
      onChange: () => data.onActivate(data.subjectIdentity),
    }),
    e(SemanticIcon, { role: data.role, token: data.iconToken, fallback: data.icon, title: data.iconName }),
    e("span", { className: "faceplate-title", title: data.label }, title),
    e("span", { className: "faceplate-role", title: data.role }, data.role),
  ),
  data.debugger && e("p", { className: "debugger-status", role: data.debugger.phase === "faulted" ? "alert" : "status" },
    e(SemanticIcon, { role: data.debugger.phase === "faulted" ? "Diagnostic" : "Status", className: "faceplate-inline-icon" }),
    data.debugger.phase === "faulted"
      ? `Fault ${data.debugger.retained_fault_code}`
      : `${data.debugger.latest_kind} · ${data.debugger.observed_count} observed`,
  ),
  data.reviewedBack && e("button", {
    type: "button",
    className: "faceplate-back-control nodrag nowheel",
    "aria-expanded": String(data.backExpanded),
    "aria-label": `${data.backExpanded ? "Close" : "Open"} reviewed Back for ${data.label}`,
    onClick: (event) => {
      event.preventDefault();
      event.stopPropagation();
      data.onOpenBack?.(data.subjectIdentity);
    },
  }, data.backExpanded ? "Close Back" : "Open Back"),
  data.clue && e("p", { className: "faceplate-clue", title: data.clue }, data.clue),
  data.ports.length > 0 && e("div", { className: "faceplate-ports", role: "group", "aria-label": "Exact typed Ports" },
    data.ports.map((port) => e(PortRow, { key: port.id, port, onActivate:data.onActivate })),
  ));
}
