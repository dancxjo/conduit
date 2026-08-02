/**
 * Conduit Patchbay UML-style component faceplates.
 *
 * Semantic ports are rendered as declared member rows. React Flow handles live
 * inside those rows so cord geometry follows the interface declaration rather
 * than an arbitrary point on the node shell.
 */

const e = window.React.createElement;

function signalFamily(typeId, nodeKind, portId) {
  const normalized = typeof typeId === "string" ? typeId.toLowerCase() : "";
  const normalizedKind = typeof nodeKind === "string" ? nodeKind.toLowerCase() : "";
  if (normalized.startsWith("conduit.net/")) {
    if (normalized.includes("link-observation")) return "network-link";
    if (normalized.endsWith("/frame")) return "network-frame";
    if (normalized.endsWith("/packet")) return "network-packet";
    if (normalized.endsWith("/datagram")) return "network-datagram";
    if (normalized.includes("byte-stream")) return "network-stream";
    if (normalized.endsWith("/session")) return "network-session";
    if (normalized.includes("control-event")) return "network-control";
    return "network-state";
  }
  if ((normalizedKind.includes("/time/") || normalizedKind.startsWith("time/")) &&
      ["tick", "pulse", "phase", "rate", "reset"].includes(portId)) {
    return "clock";
  }
  if (normalized.includes("retained-state") || normalized.includes("retained_state")) {
    return "state";
  }
  if (normalized.includes("/event") || normalized.endsWith("event")) return "event";
  if (normalized.includes("/gate") || normalized.endsWith("gate")) return "gate";
  if (normalized.includes("/control") || normalized.endsWith("control")) return "control";
  if (normalized.includes("audio")) return "audio";
  return "other";
}

function PortRow({
  nodeId,
  nodeKind,
  port,
  direction,
  isPublic,
  isConnectable,
  onPortSelect,
  onPortWatch,
}) {
  const receiving = direction === "input";
  const presentationDirection = receiving ? "receiving" : "outgoing";
  const family = signalFamily(port.type_id, nodeKind, port.id);
  const handle = e(window.ReactFlow.Handle, {
    id: port.id,
    type: receiving ? "target" : "source",
    position: receiving
      ? window.ReactFlow.Position.Left
      : window.ReactFlow.Position.Right,
    className: [
      "jack-handle",
      `signal-family-${family}`,
      isPublic ? "public-jack-handle" : "",
    ].filter(Boolean).join(" "),
    isConnectable,
    "aria-label": port.accessible_label,
    "data-semantic-path": port.semantic_path,
    "data-signal-family": family,
  });

  return e("div", {
    className: `faceplate-member-row faceplate-port-row ${presentationDirection}-jack signal-family-${family}`,
    role: "group",
    "aria-label": port.accessible_label,
    "data-semantic-path": port.semantic_path,
    "data-port-direction": presentationDirection,
    "data-signal-family": family,
  },
    receiving && e("span", {
      className: "faceplate-jack-cell receiving-jack-cell",
    }, handle),
    e("button", {
      type: "button",
      className: `jack-label ${presentationDirection}-port-label nodrag`,
      title: `${port.accessible_label}; ${family} signal; type ${port.type_id}; ` +
        `${port.delivery}; ${port.temporal}; ${port.terminal}; ${port.loss_acceptance}`,
      "aria-label": `${port.accessible_label}; type ${port.type_id}`,
      onClick: (event) => {
        event.stopPropagation();
        onPortSelect?.(nodeId, port);
      },
      onDoubleClick: (event) => {
        event.stopPropagation();
        onPortWatch?.(nodeId, port);
      },
    }, port.display_label),
    e("code", { className: "faceplate-member-type", title: port.type_id }, port.type_id),
    e("span", {
      className: "faceplate-member-state",
      title: `${port.delivery}; ${port.connections}`,
    },
      e("span", {
        className: `jack-status-dot ${port.connected ? "connected" : ""}`,
        "aria-hidden": "true",
      }),
      port.connected ? "linked" : "open",
    ),
    !receiving && e("span", {
      className: "faceplate-jack-cell outgoing-jack-cell",
    }, handle),
  );
}

export function FaceplateNodeComponent({ data, id }) {
  const {
    title,
    kind,
    inputs = [],
    outputs = [],
    status = "idle",
    activity = null,
    isComposite = false,
    isSelected = false,
    onOpenNested,
    onPortSelect,
    onPortWatch,
    validity = "valid",
    diagnosticIds = [],
    diagnosticAnchors = [],
    plannedBinding = null,
  } = data;

  const portRows = [
    ...inputs.map((port) => e(PortRow, {
      key: port.semantic_path,
      nodeId: id,
      nodeKind: kind,
      port,
      direction: "input",
      isPublic: isComposite,
      isConnectable: data.isConnectable,
      onPortSelect,
      onPortWatch,
    })),
    ...outputs.map((port) => e(PortRow, {
      key: port.semantic_path,
      nodeId: id,
      nodeKind: kind,
      port,
      direction: "output",
      isPublic: isComposite,
      isConnectable: data.isConnectable,
      onPortSelect,
      onPortWatch,
    })),
  ];

  const compactClues = [
    {
      id: "kind",
      glyph: "◇",
      label: `Semantic kind ${kind}`,
    },
    plannedBinding && {
      id: "implementation",
      glyph: "▣",
      label: `Exact implementation ${plannedBinding.implementation_id}`,
    },
    plannedBinding && {
      id: "artifact",
      glyph: "□",
      label: `Exact artifact ${plannedBinding.artifact_id}`,
    },
    plannedBinding && {
      id: "provider",
      glyph: "⌁",
      label: `Provider observation ${plannedBinding.host_observation_status}`,
    },
    plannedBinding && {
      id: "host",
      glyph: "⌂",
      label: `Exact host ${plannedBinding.host_id}`,
    },
    plannedBinding?.resources?.length > 0 && {
      id: "resource",
      glyph: "◉",
      label: `${plannedBinding.resources.length} exact resource binding${plannedBinding.resources.length === 1 ? "" : "s"}`,
    },
    plannedBinding?.allocation && {
      id: "allocation",
      glyph: "▥",
      label: "Exact finite allocation",
    },
    plannedBinding?.authorities?.length > 0 && {
      id: "authority",
      glyph: "◆",
      label: `${plannedBinding.authorities.length} exact authority binding${plannedBinding.authorities.length === 1 ? "" : "s"}`,
    },
    (activity || status === "error") && {
      id: "runtime",
      glyph: activity === "waiting" ? "Ⅱ" : activity === "active" ? "▶" : "!",
      label: `Runtime state ${activity || "error"}`,
    },
    diagnosticIds.length > 0 && {
      id: "diagnostic",
      glyph: "!",
      label: `${diagnosticIds.length} diagnostic${diagnosticIds.length === 1 ? "" : "s"}`,
    },
  ].filter(Boolean);
  const anchorRows = diagnosticAnchors.map((anchor) =>
    e("div", {
      key: anchor.id,
      className: "faceplate-member-row diagnostic-anchor-row",
      role: "note",
      "aria-label": `Rejected authored endpoint ${anchor.label}`,
      "data-diagnostic-anchor": anchor.id,
    },
      anchor.side === "to" && e(window.ReactFlow.Handle, {
        id: anchor.id,
        type: "target",
        position: window.ReactFlow.Position.Left,
        isConnectable: false,
        className: "patchbay-diagnostic-anchor-handle",
      }),
      e("strong", { className: "diagnostic-x", "aria-hidden": "true" }, "×"),
      e("span", null, anchor.label),
      anchor.side === "from" && e(window.ReactFlow.Handle, {
        id: anchor.id,
        type: "source",
        position: window.ReactFlow.Position.Right,
        isConnectable: false,
        className: "patchbay-diagnostic-anchor-handle",
      }),
    )
  );

  return e("div", {
    className: [
      "conduit-faceplate-card",
      status,
      isComposite ? "composite-faceplate" : "",
      isSelected ? "selected-faceplate" : "",
      `faceplate-validity-${validity}`,
    ].filter(Boolean).join(" "),
    tabIndex: 0,
    role: "region",
    "aria-label": [
      `Compact component faceplate ${title}; select for details`,
      validity !== "valid" ? `${validity}; ${diagnosticIds.length} diagnostics` : "",
    ].filter(Boolean).join(", "),
  },
    e("div", { className: "faceplate-header faceplate-compartment" },
      e("span", { className: "faceplate-kind-glyph", "aria-hidden": "true" }, "◇"),
      e("strong", { className: "node-title" }, title),
      e("div", { className: "faceplate-header-actions" },
        isComposite && e("span", { className: "badge composite-badge" }, "public"),
        isComposite && e("button", {
          className: "btn-icon nodrag",
          title: `Open ${title} inside`,
          "aria-label": `Open ${title} inside`,
          onClick: () => onOpenNested?.(id, kind),
        }, "↳"),
      ),
    ),
    e("div", { className: "faceplate-type-compartment faceplate-compartment" },
      e("code", { className: "kind-tag" }, kind),
      validity !== "valid" && e(
        "span",
        { className: `badge faceplate-validity-badge validity-${validity}` },
        validity,
      ),
    ),
    e("ul", {
      className: "faceplate-clues",
      "aria-label": plannedBinding ? "Compact exact realization clues" : "Compact semantic clues",
    }, compactClues.map((clue) => e("li", {
      key: clue.id,
      className: `faceplate-clue clue-${clue.id}`,
      title: clue.label,
      "aria-label": clue.label,
      "data-clue": clue.id,
    }, e("span", { "aria-hidden": "true" }, clue.glyph)))),
    e("div", {
      className: "faceplate-members-compartment faceplate-port-compartment",
      "aria-label": "Declared ports",
    }, [...portRows, ...anchorRows]),
  );
}
