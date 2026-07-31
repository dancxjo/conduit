const NODE_WIDTH = 280;
const LAYER_GAP = 80;
const TOP_MARGIN = 40;
const LEFT_MARGIN = 32;
const NODE_GAP = 56;

function compareIds(left, right) {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}

function nodeHeight(node) {
  const configRows = Object.keys(node.config || {}).length;
  const portRows = Math.max(node.inputs?.length || 0, node.outputs?.length || 0);
  return Math.max(160, 132 + Math.max(configRows * 34, portRows * 36));
}

function stableNodeIds(nodes) {
  return nodes
    .map((node) => node.id)
    .filter((id) => typeof id === "string" && id.length > 0)
    .sort(compareIds);
}

function stronglyConnectedComponents(nodeIds, outgoing) {
  let nextIndex = 0;
  const indices = new Map();
  const lowLinks = new Map();
  const stack = [];
  const onStack = new Set();
  const components = [];

  function visit(nodeId) {
    indices.set(nodeId, nextIndex);
    lowLinks.set(nodeId, nextIndex);
    nextIndex += 1;
    stack.push(nodeId);
    onStack.add(nodeId);

    for (const successor of outgoing.get(nodeId) || []) {
      if (!indices.has(successor)) {
        visit(successor);
        lowLinks.set(
          nodeId,
          Math.min(lowLinks.get(nodeId), lowLinks.get(successor)),
        );
      } else if (onStack.has(successor)) {
        lowLinks.set(
          nodeId,
          Math.min(lowLinks.get(nodeId), indices.get(successor)),
        );
      }
    }

    if (lowLinks.get(nodeId) !== indices.get(nodeId)) return;
    const component = [];
    let member;
    do {
      member = stack.pop();
      onStack.delete(member);
      component.push(member);
    } while (member !== nodeId);
    component.sort(compareIds);
    components.push(component);
  }

  for (const nodeId of nodeIds) {
    if (!indices.has(nodeId)) visit(nodeId);
  }
  return components;
}

function componentLayout(nodes, cords) {
  const nodeIds = stableNodeIds(nodes);
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const outgoing = new Map(nodeIds.map((id) => [id, new Set()]));
  for (const cord of cords) {
    if (outgoing.has(cord.from_node) && outgoing.has(cord.to_node)) {
      outgoing.get(cord.from_node).add(cord.to_node);
    }
  }

  const components = stronglyConnectedComponents(nodeIds, outgoing);
  const componentFor = new Map();
  components.forEach((component, index) => {
    for (const nodeId of component) componentFor.set(nodeId, index);
  });

  const outgoingComponents = components.map(() => new Set());
  const incomingComponents = components.map(() => new Set());
  for (const [fromNode, successors] of outgoing) {
    for (const toNode of successors) {
      const fromComponent = componentFor.get(fromNode);
      const toComponent = componentFor.get(toNode);
      if (fromComponent === toComponent) continue;
      outgoingComponents[fromComponent].add(toComponent);
      incomingComponents[toComponent].add(fromComponent);
    }
  }

  const componentName = (index) => components[index][0];
  const indegree = incomingComponents.map((incoming) => incoming.size);
  const queue = components
    .map((_component, index) => index)
    .filter((index) => indegree[index] === 0)
    .sort((left, right) => compareIds(componentName(left), componentName(right)));
  const topologicalOrder = [];
  while (queue.length > 0) {
    const component = queue.shift();
    topologicalOrder.push(component);
    for (const successor of outgoingComponents[component]) {
      indegree[successor] -= 1;
      if (indegree[successor] === 0) {
        queue.push(successor);
        queue.sort((left, right) =>
          compareIds(componentName(left), componentName(right)));
      }
    }
  }

  const ranks = components.map(() => 0);
  for (const component of topologicalOrder) {
    for (const successor of outgoingComponents[component]) {
      ranks[successor] = Math.max(ranks[successor], ranks[component] + 1);
    }
  }

  const layers = [];
  for (let index = 0; index < components.length; index += 1) {
    const rank = ranks[index];
    if (!layers[rank]) layers[rank] = [];
    layers[rank].push(index);
  }
  for (const layer of layers) {
    layer.sort((left, right) => compareIds(componentName(left), componentName(right)));
  }

  function orderMap() {
    const result = new Map();
    for (const layer of layers) {
      layer.forEach((component, index) => result.set(component, index));
    }
    return result;
  }

  function barycenter(component, neighbors, order) {
    const values = [...neighbors[component]]
      .map((neighbor) => order.get(neighbor))
      .filter((value) => value !== undefined);
    if (values.length === 0) return Number.POSITIVE_INFINITY;
    return values.reduce((sum, value) => sum + value, 0) / values.length;
  }

  function reorder(layer, neighbors, order) {
    const original = new Map(layer.map((component, index) => [component, index]));
    layer.sort((left, right) => {
      const leftCenter = barycenter(left, neighbors, order);
      const rightCenter = barycenter(right, neighbors, order);
      if (leftCenter !== rightCenter) return leftCenter - rightCenter;
      return original.get(left) - original.get(right);
    });
  }

  // Median/barycenter sweeps reduce crossings while retaining stable tie breaks.
  for (let sweep = 0; sweep < 4; sweep += 1) {
    let order = orderMap();
    for (let rank = 1; rank < layers.length; rank += 1) {
      reorder(layers[rank], incomingComponents, order);
      order = orderMap();
    }
    order = orderMap();
    for (let rank = layers.length - 2; rank >= 0; rank -= 1) {
      reorder(layers[rank], outgoingComponents, order);
      order = orderMap();
    }
  }

  const positions = new Map();
  layers.forEach((layer, rank) => {
    let y = TOP_MARGIN;
    for (const component of layer) {
      for (const nodeId of components[component]) {
        positions.set(nodeId, {
          x: LEFT_MARGIN + rank * (NODE_WIDTH + LAYER_GAP),
          y: Math.round(y),
        });
        y += nodeHeight(nodeById.get(nodeId)) + NODE_GAP;
      }
    }
  });
  return positions;
}

/**
 * Produce deterministic presentation-only MoveNode operations from the
 * projected topology. Cycles are condensed before layering, so feedback does
 * not make the arrangement depend on traversal order.
 */
export function autoArrangeOperations(viewModel) {
  const nodes = viewModel?.topology?.expanded_nodes || [];
  const cords = viewModel?.topology?.cords || [];
  const positions = componentLayout(nodes, cords);
  const maximumNodes = Math.min(
    viewModel?.bounds?.maximum_nodes || positions.size,
    positions.size,
  );

  return [...positions.entries()]
    .sort(([left], [right]) => compareIds(left, right))
    .slice(0, maximumNodes)
    .map(([nodeId, position]) => ({
      MoveNode: { node_id: nodeId, position },
    }));
}
