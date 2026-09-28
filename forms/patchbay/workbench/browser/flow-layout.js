export const MAX_LAYOUT_NODES = 512;
export const MAX_LAYOUT_EDGES = 1024;
export const LAYOUT_SWEEPS = 4;

function compare(left, right) { return left.localeCompare(right); }

function components(nodes, edges) {
  const adjacency = new Map(nodes.map((node) => [node.id, []]));
  for (const edge of edges) adjacency.get(edge.source)?.push(edge.target);
  for (const targets of adjacency.values()) targets.sort(compare);
  let sequence = 0;
  const indices = new Map(), low = new Map(), stack = [], active = new Set(), result = [];
  function visit(identity) {
    indices.set(identity, sequence); low.set(identity, sequence); sequence += 1;
    stack.push(identity); active.add(identity);
    for (const target of adjacency.get(identity)) {
      if (!indices.has(target)) { visit(target); low.set(identity, Math.min(low.get(identity), low.get(target))); }
      else if (active.has(target)) low.set(identity, Math.min(low.get(identity), indices.get(target)));
    }
    if (low.get(identity) !== indices.get(identity)) return;
    const component = [];
    while (stack.length) {
      const member = stack.pop(); active.delete(member); component.push(member);
      if (member === identity) break;
    }
    result.push(component.sort(compare));
  }
  for (const node of [...nodes].sort((a, b) => compare(a.id, b.id))) if (!indices.has(node.id)) visit(node.id);
  return result.sort((a, b) => compare(a[0], b[0]));
}

export function layoutFlowScene(nodes, edges) {
  if (nodes.length > MAX_LAYOUT_NODES || edges.length > MAX_LAYOUT_EDGES) throw new Error("Flow layout bound exceeded");
  const groups = components(nodes, edges);
  const owner = new Map(groups.flatMap((group, index) => group.map((identity) => [identity, index])));
  const outgoing = new Map(groups.map((_group, index) => [index, new Set()]));
  const incoming = new Map(groups.map((_group, index) => [index, new Set()]));
  for (const edge of edges) {
    const source = owner.get(edge.source), target = owner.get(edge.target);
    if (source === undefined || target === undefined || source === target) continue;
    outgoing.get(source).add(target); incoming.get(target).add(source);
  }
  const layers = new Map(groups.map((_group, index) => [index, 0]));
  const indegree = new Map(groups.map((_group, index) => [index, incoming.get(index).size]));
  const queue = groups.map((_group, index) => index).filter((index) => indegree.get(index) === 0)
    .sort((a, b) => compare(groups[a][0], groups[b][0]));
  while (queue.length) {
    const current = queue.shift();
    for (const target of [...outgoing.get(current)].sort((a, b) => compare(groups[a][0], groups[b][0]))) {
      layers.set(target, Math.max(layers.get(target), layers.get(current) + 1));
      indegree.set(target, indegree.get(target) - 1);
      if (indegree.get(target) === 0) queue.push(target);
    }
    queue.sort((a, b) => compare(groups[a][0], groups[b][0]));
  }
  const columns = [];
  for (let index = 0; index < groups.length; index += 1) (columns[layers.get(index)] ||= []).push(index);
  for (const column of columns) column.sort((a, b) => compare(groups[a][0], groups[b][0]));
  for (let sweep = 0; sweep < LAYOUT_SWEEPS; sweep += 1) {
    const forward = sweep % 2 === 0;
    const order = [...columns.keys()].filter((index) => columns[index]).sort((a, b) => forward ? a - b : b - a);
    const ranks = new Map(columns.flatMap((column) => column.map((group, rank) => [group, rank])));
    for (const layer of order) columns[layer].sort((left, right) => {
      const neighbors = (group) => [...(forward ? incoming.get(group) : outgoing.get(group))];
      const median = (group) => { const values = neighbors(group).map((item) => ranks.get(item)).filter(Number.isFinite).sort((a, b) => a - b); return values.length ? values[Math.floor(values.length / 2)] : ranks.get(group); };
      return median(left) - median(right) || compare(groups[left][0], groups[right][0]);
    });
  }
  const positions = new Map();
  for (let layer = 0; layer < columns.length; layer += 1) {
    let y = 90;
    for (let rank = 0; rank < (columns[layer] || []).length; rank += 1) {
      const group = groups[columns[layer][rank]];
      for (const identity of group) {
        positions.set(identity, { x: 80 + layer * 320, y });
        const node = nodes.find((candidate) => candidate.id === identity);
        y += 84 + (node?.data?.ports?.length || 0) * 28 + 40;
      }
      y += 70;
    }
  }
  return positions;
}
