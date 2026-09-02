const React = window.React;
const Flow = window.ReactFlow;
const e = React.createElement;

function PortRow({ port, onActivate }) {
  const receiving = port.direction === "receiving";
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
  return e("div", { className: `faceplate-port ${port.direction}${port.diagnosticError ? " diagnostic-error" : ""}`, "data-port-id": port.id, onClick:event=>{event.stopPropagation();onActivate(port.id);} },
    receiving && handle,
    e("span", { className: "faceplate-port-name", title: port.accessibilityName }, port.label),
    e("code", { title: port.valueKind }, port.valueKind),
    !receiving && handle,
  );
}

export function FaceplateNode({ data }) {
  const title = data.role === "Gear" ? data.label.slice(data.label.lastIndexOf("/") + 1) : data.label;
  return e("article", {
    className: `flow-faceplate role-${data.role.toLowerCase()}${data.semanticSelected ? " semantic-selected" : ""}${data.diagnosticError ? " diagnostic-error" : ""}`,
    "data-subject": data.subjectIdentity,
    "data-subject-id": data.subjectIdentity,
    "data-lens": data.lens,
    "aria-label": data.accessibilityName,
    "aria-pressed": String(data.semanticSelected),
    role: "button",
    tabIndex: 0,
    onClick: (event) => {
      event.stopPropagation();
      data.onActivate(data.subjectIdentity);
    },
    onKeyDown: (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      event.stopPropagation();
      data.onActivate(data.subjectIdentity);
    },
  },
  e("header", null,
    e("span", { className: "faceplate-icon", title: data.iconName, "aria-hidden": "true" }, data.icon),
    e("span", { className: "faceplate-title", title: data.label }, title),
    e("span", { className: "faceplate-role", title: data.role }, data.role),
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
