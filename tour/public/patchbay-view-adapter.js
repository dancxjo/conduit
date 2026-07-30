/**
 * Conduit Patchbay View Adapter
 *
 * Converts authoritative Conduit .panel source and runtime state into
 * deterministic Patchbay View Models for renderer presentation.
 */

export function parsePanelToViewModel(sourceText, runtimeState = {}, savedPositions = {}) {
  const nodes = [];
  const edges = [];

  if (!sourceText || typeof sourceText !== "string") {
    return { nodes, edges, diagnostics: [] };
  }

  const lines = sourceText.split("\n");
  const nodeMap = new Map();
  const composites = new Map();
  const cords = [];

  let currentComposite = null;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();

    // Composite header
    const compMatch = trimmed.match(/^composite\s+([A-Za-z0-9_\/-]+)\s*\{/);
    if (compMatch) {
      currentComposite = {
        name: compMatch[1],
        inputs: [],
        outputs: [],
        nodes: []
      };
      composites.set(compMatch[1], currentComposite);
      continue;
    }

    if (currentComposite) {
      if (trimmed === "}") {
        currentComposite = null;
        continue;
      }
      const expInMatch = trimmed.match(/^export\s+input\s+([A-Za-z0-9_-]+)/);
      if (expInMatch) {
        currentComposite.inputs.push(expInMatch[1]);
        continue;
      }
      const expOutMatch = trimmed.match(/^export\s+output\s+([A-Za-z0-9_-]+)/);
      if (expOutMatch) {
        currentComposite.outputs.push(expOutMatch[1]);
        continue;
      }
    }

    // Node definition with optional block config
    const nodeMatch = trimmed.match(/^node\s+([A-Za-z0-9_-]+)\s*:\s*([A-Za-z0-9_\/-]+)(?:\s*\{)?/);
    if (nodeMatch && !currentComposite) {
      const id = nodeMatch[1];
      const kind = nodeMatch[2];
      const config = {};

      // Look ahead for config attributes inside block
      if (line.includes("{")) {
        let j = i + 1;
        while (j < lines.length && !lines[j].trim().startsWith("}")) {
          const attrMatch = lines[j].trim().match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/);
          if (attrMatch) {
            let val = attrMatch[2].trim();
            if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
              val = val.slice(1, -1);
            }
            config[attrMatch[1]] = val;
          }
          j++;
        }
      }

      nodeMap.set(id, { id, kind, config });
    }

    // Cord definition
    const cordMatch = trimmed.match(/^cord\s+([A-Za-z0-9_-]+)\.([A-Za-z0-9_-]+)\s*->\s*([A-Za-z0-9_-]+)\.([A-Za-z0-9_-]+)(?:\s*\{)?/);
    if (cordMatch) {
      const cord = {
        id: `cord_${cords.length}_${cordMatch[1]}_${cordMatch[3]}`,
        sourceNodeId: cordMatch[1],
        sourcePortId: cordMatch[2],
        targetNodeId: cordMatch[3],
        targetPortId: cordMatch[4],
        capacity: 8,
        pressure: "block"
      };

      if (line.includes("{")) {
        let j = i + 1;
        while (j < lines.length && !lines[j].trim().startsWith("}")) {
          const capMatch = lines[j].trim().match(/^capacity\s*=\s*(\d+)/);
          if (capMatch) cord.capacity = parseInt(capMatch[1], 10);
          const pressMatch = lines[j].trim().match(/^pressure\s*=\s*([A-Za-z0-9_-]+)/);
          if (pressMatch) cord.pressure = pressMatch[1];
          j++;
        }
      }
      cords.push(cord);
    }
  }

  // Build Node View Models
  let index = 0;
  nodeMap.forEach((node, id) => {
    const isComposite = composites.has(node.kind);
    const compDef = composites.get(node.kind);

    let inputs = [];
    let outputs = [];

    if (isComposite) {
      inputs = compDef.inputs.map(p => ({
        id: `${id}.${p}`,
        name: p,
        direction: "input",
        type: "conduit/any",
        required: true,
        connectionState: cords.some(c => c.targetNodeId === id && c.targetPortId === p) ? "connected" : "disconnected"
      }));
      outputs = compDef.outputs.map(p => ({
        id: `${id}.${p}`,
        name: p,
        direction: "output",
        type: "conduit/any",
        required: false,
        connectionState: cords.some(c => c.sourceNodeId === id && c.sourcePortId === p) ? "connected" : "disconnected"
      }));
    } else {
      // Standard Node Contracts
      if (node.kind.includes("literal")) {
        outputs = [{ id: `${id}.out`, name: "out", direction: "output", type: "conduit/text", required: false, connectionState: cords.some(c => c.sourceNodeId === id) ? "connected" : "disconnected" }];
      } else if (node.kind.includes("stdout") || node.kind.includes("log")) {
        inputs = [{ id: `${id}.in`, name: "in", direction: "input", type: "conduit/text", required: true, connectionState: cords.some(c => c.targetNodeId === id) ? "connected" : "disconnected" }];
      } else if (node.kind.includes("file-read") || node.kind.includes("wifi") || node.kind.includes("udp")) {
        outputs = [{ id: `${id}.out`, name: "out", direction: "output", type: "conduit/stream", required: false, connectionState: cords.some(c => c.sourceNodeId === id) ? "connected" : "disconnected" }];
      } else if (node.kind.includes("file-write")) {
        inputs = [{ id: `${id}.in`, name: "in", direction: "input", type: "conduit/stream", required: true, connectionState: cords.some(c => c.targetNodeId === id) ? "connected" : "disconnected" }];
      } else if (node.kind.includes("http-server")) {
        outputs = [
          { id: `${id}.out`, name: "out", direction: "output", type: "conduit/http-req", required: false, connectionState: cords.some(c => c.sourceNodeId === id && c.sourcePortId === "out") ? "connected" : "disconnected" },
          { id: `${id}.errors`, name: "errors", direction: "output", type: "conduit/error", required: false, connectionState: "disconnected" }
        ];
      } else if (node.kind.includes("http-client")) {
        inputs = [{ id: `${id}.in`, name: "in", direction: "input", type: "conduit/http-req", required: true, connectionState: cords.some(c => c.targetNodeId === id) ? "connected" : "disconnected" }];
        outputs = [{ id: `${id}.out`, name: "out", direction: "output", type: "conduit/http-res", required: false, connectionState: cords.some(c => c.sourceNodeId === id) ? "connected" : "disconnected" }];
      } else {
        inputs = [{ id: `${id}.in`, name: "in", direction: "input", type: "conduit/any", required: true, connectionState: cords.some(c => c.targetNodeId === id) ? "connected" : "disconnected" }];
        outputs = [{ id: `${id}.out`, name: "out", direction: "output", type: "conduit/any", required: false, connectionState: cords.some(c => c.sourceNodeId === id) ? "connected" : "disconnected" }];
      }
    }

    const defaultPos = {
      x: 80 + (index % 2) * 340,
      y: 60 + Math.floor(index / 2) * 240
    };

    nodes.push({
      id,
      title: id,
      kind: node.kind,
      config: node.config,
      position: savedPositions[id] || defaultPos,
      inputs,
      outputs,
      isComposite,
      status: runtimeState[id]?.status || "idle",
      metrics: runtimeState[id]?.metrics || null
    });

    index++;
  });

  // Build Edge View Models
  cords.forEach(c => {
    edges.push({
      id: c.id,
      sourceNodeId: c.sourceNodeId,
      sourcePortId: c.sourcePortId,
      targetNodeId: c.targetNodeId,
      targetPortId: c.targetPortId,
      capacity: c.capacity,
      pressure: c.pressure
    });
  });

  return { nodes, edges };
}
