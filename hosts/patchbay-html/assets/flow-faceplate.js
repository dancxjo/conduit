const React = window.React;
const Flow = window.ReactFlow;
const e = React.createElement;

function PortRow({ port }) {
  const receiving = port.direction === "receiving";
  const handle = e(Flow.Handle, {
    id: port.id,
    type: receiving ? "target" : "source",
    position: receiving ? Flow.Position.Left : Flow.Position.Right,
    isConnectable: false,
    className: "faceplate-handle",
    "aria-label": port.accessibilityName,
    "data-port-id": port.id,
    "data-port-direction": port.direction,
  });
  return e("div", { className: `faceplate-port ${port.direction}`, "data-port-id": port.id },
    receiving && handle,
    e("span", { className: "faceplate-port-name", title: port.accessibilityName }, port.label),
    e("code", { title: port.valueKind }, port.valueKind),
    !receiving && handle,
  );
}

export function FaceplateNode({ data }) {
  return e("article", {
    className: `flow-faceplate role-${data.role.toLowerCase()}`,
    "data-subject-id": data.subjectIdentity,
    "data-lens": data.lens,
    "aria-label": data.accessibilityName,
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
    e("span", { className: "faceplate-title", title: data.label }, data.label),
    e("span", { className: "faceplate-role" }, data.role),
  ),
  data.clue && e("p", { className: "faceplate-clue", title: data.clue }, data.clue),
  data.ports.length > 0 && e("div", { className: "faceplate-ports", "aria-label": "Exact typed Ports" },
    data.ports.map((port) => e(PortRow, { key: port.id, port })),
  ));
}
