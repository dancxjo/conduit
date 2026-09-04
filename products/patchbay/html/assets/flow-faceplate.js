const React = window.React;
const Flow = window.ReactFlow;
const e = React.createElement;

function PortRow({ port, onActivate }) {
  const receiving = port.direction === "receiving";
  const activity = port.debugger;
  const handle = e(Flow.Handle, {
    id: port.id,
    type: receiving ? "target" : "source",
    position: receiving ? Flow.Position.Left : Flow.Position.Right,
    isConnectable: true,
    className: "faceplate-handle",
    "aria-label": port.accessibilityName,
    "data-port-id": port.id,
    "data-port-direction": port.direction,
  });
  return e("div", { className: `faceplate-port ${port.direction}${port.diagnosticError ? " diagnostic-error" : ""}${activity ? ` debugger-${activity.phase}` : ""}`, "data-port-id": port.id, "data-debugger-phase": activity?.phase || "inactive", onClick:event=>{event.stopPropagation();onActivate(port.id);} },
    receiving && handle,
    e("span", { className: "faceplate-port-name", title: port.accessibilityName }, port.label),
    e("code", { title: port.valueKind }, port.valueKind),
    activity?.latest_value && e("output", { className: "debugger-value", "aria-label": `Latest observed value ${activity.latest_value.summary}` }, activity.latest_value.summary),
    !receiving && handle,
  );
}

export function FaceplateNode({ data }) {
  const title = data.role === "Gear" ? data.label.slice(data.label.lastIndexOf("/") + 1) : data.label;
  return e("article", {
    className: `flow-faceplate role-${data.role.toLowerCase()}${data.semanticSelected ? " semantic-selected" : ""}${data.diagnosticError ? " diagnostic-error" : ""}${data.debugger ? ` debugger-${data.debugger.phase}` : ""}`,
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
    e("span", { className: "faceplate-icon", title: data.iconName, "aria-hidden": "true" }, data.icon),
    e("span", { className: "faceplate-title", title: data.label }, title),
    e("span", { className: "faceplate-role", title: data.role }, data.role),
  ),
  data.debugger && e("p", { className: "debugger-status", role: data.debugger.phase === "faulted" ? "alert" : "status" },
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
  data.ports.length > 0 && e("div", { className: "faceplate-ports", "aria-label": "Exact typed Ports" },
    data.ports.map((port) => e(PortRow, { key: port.id, port, onActivate:data.onActivate })),
  ));
}
