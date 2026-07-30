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

  const handleInputChange = (key, val) => {
    const next = { ...configValues, [key]: val };
    setConfigValues(next);
    if (onConfigChange) {
      onConfigChange(id, key, val);
    }
  };

  const statusColor = status === "running" ? "#22c55e" : status === "error" ? "#ef4444" : "#94a3b8";

  // Build input handles (left)
  const inputJacks = inputs.map((port, idx) => {
    const handleId = port.name;
    const topOffset = 60 + idx * 36;

    return e("div", { key: port.id, className: "faceplate-jack input-jack", style: { top: `${topOffset}px` } },
      e(window.ReactFlow.Handle, {
        type: "target",
        position: window.ReactFlow.Position.Left,
        id: handleId,
        className: `jack-handle ${port.connectionState}`,
        style: { top: "50%", background: port.connectionState === "connected" ? "#38bdf8" : "#64748b" }
      }),
      e("span", { className: "jack-label", title: `Type: ${port.type}` },
        e("span", { className: "jack-status-dot", style: { background: port.connectionState === "connected" ? "#38bdf8" : "#475569" } }),
        port.name
      )
    );
  });

  // Build output handles (right)
  const outputJacks = outputs.map((port, idx) => {
    const handleId = port.name;
    const topOffset = 60 + idx * 36;

    return e("div", { key: port.id, className: "faceplate-jack output-jack", style: { top: `${topOffset}px` } },
      e("span", { className: "jack-label", title: `Type: ${port.type}` },
        port.name,
        e("span", { className: "jack-status-dot", style: { background: port.connectionState === "connected" ? "#38bdf8" : "#475569" } })
      ),
      e(window.ReactFlow.Handle, {
        type: "source",
        position: window.ReactFlow.Position.Right,
        id: handleId,
        className: `jack-handle ${port.connectionState}`,
        style: { top: "50%", background: port.connectionState === "connected" ? "#38bdf8" : "#64748b" }
      })
    );
  });

  // Config controls
  const configFields = Object.keys(configValues).map((key) => {
    return e("div", { key, className: "faceplate-control-row" },
      e("label", { className: "control-label" }, key),
      e("input", {
        type: "text",
        className: "control-input nodrag",
        value: configValues[key] || "",
        onMouseDown: (evt) => evt.stopPropagation(),
        onChange: (evt) => handleInputChange(key, evt.target.value)
      })
    );
  });

  // Default fields for known node types if not explicitly set
  if (configFields.length === 0 && expanded) {
    if (kind.includes("literal")) {
      configFields.push(
        e("div", { key: "value", className: "faceplate-control-row" },
          e("label", { className: "control-label" }, "value"),
          e("input", {
            type: "text",
            className: "control-input nodrag",
            placeholder: "Literal string...",
            onMouseDown: (evt) => evt.stopPropagation(),
            onChange: (evt) => handleInputChange("value", evt.target.value)
          })
        )
      );
    } else if (kind.includes("http-server")) {
      configFields.push(
        e("div", { key: "port", className: "faceplate-control-row" },
          e("label", { className: "control-label" }, "port"),
          e("input", {
            type: "text",
            className: "control-input nodrag",
            defaultValue: "8080",
            onMouseDown: (evt) => evt.stopPropagation(),
            onChange: (evt) => handleInputChange("port", evt.target.value)
          })
        )
      );
    } else if (kind.includes("file-read") || kind.includes("file-write")) {
      configFields.push(
        e("div", { key: "path", className: "faceplate-control-row" },
          e("label", { className: "control-label" }, "path"),
          e("input", {
            type: "text",
            className: "control-input nodrag",
            placeholder: "/var/data.log",
            onMouseDown: (evt) => evt.stopPropagation(),
            onChange: (evt) => handleInputChange("path", evt.target.value)
          })
        )
      );
    }
  }

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
      e("span", { className: "badge placement-tag" }, "dedicated-worker")
    ),
    // Body
    expanded && e("div", { className: "faceplate-body" },
      configFields,
      // Activity meter
      (kind.includes("stdout") || kind.includes("log") || kind.includes("http")) && e("div", { className: "faceplate-meter" },
        e("span", { className: "meter-label" }, "Activity:"),
        e("span", { className: "sparkline" }, "▂▃▅▂▇ 184 msg/s")
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
