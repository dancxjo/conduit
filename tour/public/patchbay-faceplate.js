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
    collapsed = false,
    onCollapseChange,
    validity = "valid",
    diagnosticIds = [],
    diagnosticAnchors = [],
    plannedBinding = null,
    logicalOrigin = null,
    compositeProvenance = [],
    readOnly = false,
    contract_identity: contractIdentity = null,
    semantic_effects: semanticEffects = [],
  } = data;

  const [expanded, setExpanded] = window.React.useState(!collapsed);
  const [configValues, setConfigValues] = window.React.useState(config);
  const updateNodeInternals = window.ReactFlow.useUpdateNodeInternals();

  window.React.useEffect(() => {
    setConfigValues(config);
  }, [config]);

  window.React.useEffect(() => {
    setExpanded(!collapsed);
  }, [collapsed]);

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
        readOnly: readOnly || !projection.editable,
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
  const budgetValue = plannedBinding && [
    ["memory", plannedBinding.allocation?.memory_bytes],
    ["storage", plannedBinding.allocation?.storage_bytes],
    ["cpu", plannedBinding.allocation?.cpu_units],
    ["timers", plannedBinding.allocation?.timers],
    ["transports", plannedBinding.allocation?.transports],
    ["checkpoints", plannedBinding.allocation?.checkpoints],
    ["evidence", plannedBinding.allocation?.evidence_bytes],
  ].map(([label, value]) => `${label}=${value ?? 0}`).join(", ");
  const plannedFacts = plannedBinding ? [
    { label: "planned instance", value: plannedBinding.instance },
    { label: "source origin", value: logicalOrigin || plannedBinding.logical_origin },
    compositeProvenance.length > 0 && {
      label: "composite provenance",
      value: compositeProvenance.join(" → "),
    },
    {
      label: "semantic contract",
      value: `${plannedBinding.contract_id} · ${plannedBinding.contract_identity}`,
    },
    {
      label: "implementation",
      value: `${plannedBinding.implementation_id} · ${plannedBinding.implementation_identity}`,
    },
    {
      label: "lifecycle policy",
      value: `${plannedBinding.lifecycle_policy_id} · ${plannedBinding.lifecycle_policy_identity}`,
    },
    {
      label: "artifact",
      value: `${plannedBinding.artifact_id} · ${plannedBinding.artifact_digest}`,
    },
    { label: "host", value: plannedBinding.host_id },
    {
      label: "host observation",
      value: `${plannedBinding.host_observation_id} · ` +
        `${plannedBinding.host_observation_identity}; ` +
        `${plannedBinding.host_observation_time_basis} ` +
        `${plannedBinding.host_observed_at_tick}–${plannedBinding.host_valid_until_tick}; ` +
        `${plannedBinding.host_observation_status}`,
    },
    {
      label: "availability",
      value: `${plannedBinding.availability_state} · ${plannedBinding.reason_code}`,
    },
    { label: "allocation", value: budgetValue },
    ...(plannedBinding.resources || []).map((resource) => ({
      label: "resource",
      value: `${resource.resource_kind}/${resource.resource_id} · ` +
        `${resource.binding_id} · observation ${resource.host_observation_id}` +
        (resource.lease_id ? ` · lease ${resource.lease_id}` : ""),
    })),
    ...(plannedBinding.authorities || []).map((authority) => ({
      label: "authority",
      value: `${authority.action} ${authority.resource_kind}` +
        (authority.resource_id ? `/${authority.resource_id}` : "") +
        ` · grant ${authority.grant_id} · capability ${authority.capability_id}` +
        (authority.check_at_use ? " · checked at use" : ""),
    })),
  ].filter(Boolean) : [];
  const semanticPromiseFacts = !plannedBinding ? [
    {
      label: "semantic contract",
      value: contractIdentity ? `${kind} · ${contractIdentity}` : `${kind} · source-owned boundary`,
    },
    {
      label: "semantic effects",
      value: semanticEffects.length > 0 ? semanticEffects.join(", ") : "none declared",
    },
    ...[...inputs, ...outputs].map((port) => ({
      label: `${port.direction} ${port.id}`,
      value: `${port.delivery}; ${port.temporal}; ${port.terminal}; ` +
        `${port.values}; ${port.presence}; ${port.loss_acceptance}`,
    })),
  ] : [];
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
          onClick: () => {
            setExpanded(!expanded);
            onCollapseChange?.(id, expanded);
          },
        }, expanded ? "−" : "+"),
      ),
    ),
    e("div", { className: "faceplate-type-compartment faceplate-compartment" },
      e("code", { className: "kind-tag" }, kind),
      plannedBinding && e("span", { className: "badge" }, "read-only plan"),
      validity !== "valid" && e(
        "span",
        { className: `badge faceplate-validity-badge validity-${validity}` },
        validity,
      ),
    ),
    expanded && semanticPromiseFacts.length > 0 && e("div", {
      className: "faceplate-status-compartment faceplate-compartment semantic-promise-compartment",
      "aria-label": "Semantic promises",
    },
      ...semanticPromiseFacts.map((fact, index) =>
        e("div", {
          key: `${fact.label}-${index}`,
          className: "faceplate-status-row",
        },
          e("span", { className: "faceplate-status-label" }, fact.label),
          e("code", { className: "planned-realization-value" }, fact.value),
        )
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
    expanded && plannedFacts.length > 0 && e("div", {
      className: "faceplate-status-compartment faceplate-compartment planned-realization-compartment",
      "aria-label": "Exact planned realization",
    },
      ...plannedFacts.map((fact, index) =>
        e("div", {
          key: `${fact.label}-${index}`,
          className: "faceplate-status-row planned-realization-row",
        },
          e("span", { className: "faceplate-status-label" }, fact.label),
          e("code", { className: "planned-realization-value" }, fact.value),
        )
      ),
    ),
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
        onClick: () => onOpenNested?.(id, kind),
        "aria-label": `Open ${title} inside`,
      }, "Inside"),
    ),
  );
}
