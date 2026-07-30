/**
 * Conduit Patchbay Rich Equipment Faceplate Renderer (#90, #99, #91)
 *
 * Implements standard customizable equipment faceplate DOM components
 * for ReactFlow projection.
 */

const e = window.React.createElement;

export function FaceplateNodeComponent({ data, id }) {
  const {
    title,
    kind,
    config = {},
    inputs = [],
    outputs = [],
    status = "idle",
    placement = null,
    activity = null,
    isComposite = false,
    isSelected = false,
    onConfigChange,
    onOpenNested
  } = data;

  const [expanded, setExpanded] = window.React.useState(true);
  const [configValues, setConfigValues] = window.React.useState(config);

  window.React.useEffect(() => {
    setConfigValues(config);
  }, [config]);

  const handleInputChange = (key, projection, val) => {
    const next = {
      ...configValues,
      [key]: { ...projection, display_value: val }
    };
    setConfigValues(next);
    if (onConfigChange) {
      onConfigChange(id, key, val, projection.kind);
    }
  };

  const statusColor = status === "running" ? "#22c55e" : status === "error" ? "#ef4444" : "#94a3b8";

  // ReactFlow owns port geometry. Type and connection facts come from Rust.
  const inputJacks = inputs.map((port, idx) => {
    const topOffset = 60 + idx * 36;

    return e("div", { key: port.id, className: "faceplate-jack input-jack", style: { top: `${topOffset}px` } },
      e(window.ReactFlow.Handle, {
        id: port.id,
        type: "target",
        position: window.ReactFlow.Position.Left,
        className: "jack-handle",
        isConnectable: false,
      }),
      e("span", { className: "jack-label", title: `Type: ${port.type_id}` },
        e("span", { className: "jack-status-dot", style: { background: port.connected ? "#38bdf8" : "#475569" } }),
        port.id
      )
    );
  });

  // Build output handles (right)
  const outputJacks = outputs.map((port, idx) => {
    const topOffset = 60 + idx * 36;

    return e("div", { key: port.id, className: "faceplate-jack output-jack", style: { top: `${topOffset}px` } },
      e(window.ReactFlow.Handle, {
        id: port.id,
        type: "source",
        position: window.ReactFlow.Position.Right,
        className: "jack-handle",
        isConnectable: false,
      }),
      e("span", { className: "jack-label", title: `Type: ${port.type_id}` },
        port.id,
        e("span", { className: "jack-status-dot", style: { background: port.connected ? "#38bdf8" : "#475569" } })
      )
    );
  });

  // Config controls
  const configFields = Object.keys(configValues).map((key) => {
    const projection = configValues[key];
    return e("div", { key, className: "faceplate-control-row" },
      e("label", { className: "control-label" }, key),
      e("input", {
        type: "text",
        className: "control-input nodrag",
        value: projection.display_value,
        readOnly: !projection.editable,
        onMouseDown: (evt) => evt.stopPropagation(),
        onChange: (evt) => handleInputChange(key, projection, evt.target.value)
      })
    );
  });

  return e("div", {
      className: `conduit-faceplate-card ${status} ${isComposite ? "composite-faceplate" : ""} ${isSelected ? "selected-faceplate" : ""}`,
      tabIndex: 0,
      role: "region",
      "aria-label": `Equipment faceplate ${title}`
    },
    // Header
    e("div", { className: "faceplate-header" },
      e("div", { className: "faceplate-title-group" },
        e("span", { className: "status-led", style: { background: statusColor }, title: `Status: ${status}` }),
        e("strong", { className: "node-title" }, title)
      ),
      e("div", { className: "faceplate-header-actions" },
        isComposite && e("span", { className: "badge composite-badge" }, "Panel Surface"),
        e("button", {
          className: "btn-icon nodrag",
          title: expanded ? "Collapse Faceplate" : "Expand Faceplate",
          onClick: () => setExpanded(!expanded)
        }, expanded ? "▲" : "▼")
      )
    ),
    // Subheader
    e("div", { className: "faceplate-subhead" },
      e("code", { className: "kind-tag" }, kind),
      placement && e("span", { className: "badge placement-tag" }, placement)
    ),
    // Body
    expanded && e("div", { className: "faceplate-body" },
      configFields,
      activity && e("div", { className: "faceplate-meter" },
        e("span", { className: "meter-label" }, "Activity:"),
        e("span", { className: "sparkline" }, activity)
      ),
      // Composite panel inspection action
      isComposite && e("div", { className: "composite-action-row" },
        e("button", {
          className: "btn small secondary nodrag",
          onClick: () => onOpenNested && onOpenNested(kind)
        }, "🔍 Inspect Nested Surface")
      )
    ),
    // Jacks
    inputJacks,
    outputJacks
  );
}
