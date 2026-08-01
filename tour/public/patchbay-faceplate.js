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
      title: `${port.accessible_label}; ${family} signal; type ${port.type_id}`,
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
    e("code", { className: "faceplate-member-type" }, port.type_id),
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
    config = {},
    inputs = [],
    outputs = [],
    status = "idle",
    placement = null,
    availability = null,
    activity = null,
    isComposite = false,
    isSelected = false,
    onConfigChange,
    onOpenNested,
    onPortSelect,
    onPortWatch,
    validity = "valid",
    diagnosticIds = [],
    diagnosticAnchors = [],
  } = data;

  const [expanded, setExpanded] = window.React.useState(true);
  const [configValues, setConfigValues] = window.React.useState(config);
  const updateNodeInternals = window.ReactFlow.useUpdateNodeInternals();

  window.React.useEffect(() => {
    setConfigValues(config);
  }, [config]);

  window.React.useEffect(() => {
    const frame = window.requestAnimationFrame(() => updateNodeInternals(id));
    return () => window.cancelAnimationFrame(frame);
  }, [expanded, id, updateNodeInternals]);

  const handleInputChange = (key, projection, value) => {
    const next = {
      ...configValues,
      [key]: { ...projection, display_value: value },
    };
    setConfigValues(next);
    onConfigChange?.(id, key, value, projection.kind);
  };

  const configRows = Object.entries(configValues).map(([key, projection]) =>
    e("div", {
      key,
      className: "faceplate-member-row faceplate-config-row",
      "data-config-key": key,
    },
      e("label", { className: "control-label", htmlFor: `${id}-config-${key}` }, key),
      e("input", {
        id: `${id}-config-${key}`,
        type: "text",
        className: "control-input nodrag",
        value: projection.display_value,
        readOnly: !projection.editable,
        onMouseDown: (event) => event.stopPropagation(),
        onChange: (event) =>
          handleInputChange(key, projection, event.target.value),
      }),
      e("code", { className: "faceplate-member-type" }, projection.kind),
      e("span", { className: "faceplate-member-state" },
        projection.editable ? "config" : "fixed",
      ),
    )
  );

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

  const statusFacts = [
    availability && {
      label: "provider",
      value: availability.availability_state,
      title: availability.reason_code,
    },
    placement && { label: "placement", value: placement },
    activity && { label: "execution", value: activity },
    !activity && status === "error" && { label: "execution", value: "error" },
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
      `Component faceplate ${title}`,
      validity !== "valid" ? `${validity}; ${diagnosticIds.length} diagnostics` : "",
    ].filter(Boolean).join(", "),
  },
    e("div", { className: "faceplate-header faceplate-compartment" },
      e("strong", { className: "node-title" }, title),
      e("div", { className: "faceplate-header-actions" },
        isComposite && e("span", { className: "badge composite-badge" }, "public"),
        e("button", {
          className: "btn-icon nodrag",
          title: expanded ? "Collapse Faceplate" : "Expand Faceplate",
          "aria-expanded": String(expanded),
          onClick: () => setExpanded(!expanded),
        }, expanded ? "−" : "+"),
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
    expanded && configRows.length > 0 && e("div", {
      className: "faceplate-members-compartment faceplate-config-compartment",
      "aria-label": "Configuration values",
    }, configRows),
    e("div", {
      className: "faceplate-members-compartment faceplate-port-compartment",
      "aria-label": "Declared ports",
    }, [...portRows, ...anchorRows]),
    expanded && statusFacts.length > 0 && e("div", {
      className: "faceplate-status-compartment faceplate-compartment",
      "aria-label": "Component status",
    },
      ...statusFacts.map((fact) =>
        e("div", { key: `${fact.label}-${fact.value}`, className: "faceplate-status-row" },
          e("span", { className: "faceplate-status-label" }, fact.label),
          e("span", { className: "badge availability-tag", title: fact.title }, fact.value),
        )
      ),
      isComposite && e("button", {
        className: "btn small secondary nodrag faceplate-inspect-action",
        onClick: () => onOpenNested?.(kind),
      }, "Inspect surface"),
    ),
  );
}
