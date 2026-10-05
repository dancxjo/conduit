const React = window.React;
const e = React.createElement;

export function WorkspaceDecoration({ data }) {
  return e("aside", { className: `workspace-${data.kind}`, "aria-label": `${data.kind}: ${data.title}` },
    e("strong", null, data.title),
    data.kind === "frame" && e("small", null, `Presentation frame${data.collapsed ? " (collapsed)" : ""} — not a semantic scope`),
  );
}

export function workspaceDecorations(layout) {
  if (!layout) return [];
  return [
    ...layout.frames.map((frame) => ({
      id: `workspace-frame:${frame.id}`, type: "workspaceDecoration",
      position: { x: frame.x, y: frame.y }, draggable: false, selectable: false, connectable: false,
      zIndex: -1, style: { width: frame.width, height: frame.collapsed ? 56 : frame.height },
      data: { kind: "frame", title: frame.title, collapsed: frame.collapsed },
    })),
    ...layout.notes.map((note) => ({
      id: `workspace-note:${note.id}`, type: "workspaceDecoration",
      position: { x: note.x, y: note.y }, draggable: false, selectable: false, connectable: false,
      data: { kind: "note", title: note.text },
    })),
  ];
}

export function WorkspaceManualEdge({ id, sourceX, sourceY, targetX, targetY, data, markerEnd, style, label }) {
  const points = data?.workspaceRoute?.points || [];
  const path = `M ${sourceX} ${sourceY} ${points.map((p) => `L ${p.x} ${p.y}`).join(" ")} L ${targetX} ${targetY}`;
  return e("g", null,
    e("path", { id, d: path, markerEnd, style, fill: "none", className: "react-flow__edge-path" }),
    label && e("text", { x: (sourceX + targetX) / 2, y: (sourceY + targetY) / 2 - 8,
      textAnchor: "middle", className: "react-flow__edge-text" }, label),
  );
}
