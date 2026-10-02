// A Mask over exact resident graph facts. No parsing, planning, or graph edits.
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const SCHEMA = 'conduit.presentation/graph-canvas@1';
const SVG = 'http://www.w3.org/2000/svg';
export function decodeGraphCanvas(source) {
  const bytes = encoder.encode(source);
  if (bytes.length > 65536) throw new Error('Graph canvas exceeds its byte bound');
  let offset = 0;
  const field = () => {
    const end = bytes.indexOf(58, offset);
    if (end < offset || end - offset > 4) throw new Error('Malformed graph field');
    const prefix = decoder.decode(bytes.subarray(offset, end));
    if (!/^\d{1,4}$/.test(prefix)) throw new Error('Malformed graph length');
    const size = Number(prefix); offset = end + 1;
    if (size > 1024 || offset + size > bytes.length) throw new Error('Graph field exceeds its bound');
    const value = decoder.decode(bytes.subarray(offset, offset + size)); offset += size; return value;
  };
  const records = (maximum, names) => {
    const count = field();
    if (!/^\d+$/.test(count) || Number(count) > maximum) throw new Error('Graph record count exceeds its bound');
    return Array.from({ length: Number(count) }, () => Object.fromEntries(names.map(name => [name, field()])));
  };
  if (field() !== SCHEMA) throw new Error('Unknown graph canvas schema');
  const graph = Object.fromEntries(['checked_plot','expanded_plot','plan','body_plan','play','selected'].map(name => [name, field()]));
  graph.nodes = records(128, ['identity','label','kind']);
  graph.ports = records(512, ['identity','owner','label','direction','info','temporal']);
  graph.cords = records(512, ['identity','source','sink']);
  if (offset !== bytes.length || ['checked_plot','expanded_plot','plan','body_plan'].some(key => !graph[key])) throw new Error('Malformed graph basis');
  const subjects = new Set(), nodes = new Set(graph.nodes.map(node => node.identity));
  for (const item of [...graph.nodes, ...graph.ports, ...graph.cords]) {
    if (!item.identity || subjects.has(item.identity)) throw new Error('Duplicate or empty graph identity');
    subjects.add(item.identity);
  }
  for (const port of graph.ports) if ((port.owner && !nodes.has(port.owner)) || !['input','output'].includes(port.direction)) throw new Error('Unknown graph port owner or direction');
  const ports = new Set(graph.ports.map(port => port.identity));
  for (const cord of graph.cords) if (!ports.has(cord.source) || !ports.has(cord.sink)) throw new Error('Unknown graph cord endpoint');
  if (graph.selected && !subjects.has(graph.selected)) throw new Error('Unknown graph selection');
  return graph;
}

export function renderGraphCanvas(element, source, select) {
  const graph = decodeGraphCanvas(source);
  const document = element.ownerDocument;
  element.replaceChildren();
  element.dataset.checkedPlotId = graph.checked_plot;
  element.dataset.expandedPlotId = graph.expanded_plot;
  element.dataset.planId = graph.plan;
  element.dataset.bodyPlanId = graph.body_plan;
  element.dataset.activePlayId = graph.play;
  element.style.overflow = 'auto';
  const hint = document.createElement('p');
  hint.textContent = 'Follow a cord from an output to an input. Select a gear, port, or cord to inspect its exact facts. Tab moves between subjects; Enter selects.';
  element.append(hint);
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('role', 'group'); svg.setAttribute('aria-label', 'Resident plot gears, ports, and cords');
  const create = (tag, attrs, parent = svg) => {
    const node = document.createElementNS(SVG, tag);
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, String(value));
    parent.append(node); return node;
  };
  const selectable = (node, identity, label) => {
    node.dataset.subjectIdentity = identity;
    node.setAttribute('tabindex', '0'); node.setAttribute('role', 'button');
    node.setAttribute('aria-label', label); node.setAttribute('aria-pressed', String(identity === graph.selected));
    node.style.cursor = 'pointer';
    node.addEventListener('click', () => select(identity));
    node.addEventListener('keydown', event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); select(identity); } });
  };
  const models = graph.nodes.map(node => ({ ...node, ports: graph.ports.filter(port => port.owner === node.identity) }));
  const boundaryPorts = graph.ports.filter(port => !port.owner);
  if (boundaryPorts.length) models.unshift({ identity: '', label: 'Plot boundary', kind: 'Inputs and outputs', ports: boundaryPorts });
  const positions = new Map();
  let y = 32;
  for (const [i, node] of models.entries()) {
    const x = 40 + (i % 2) * 420;
    const height = 92 + node.ports.length * 26;
    if (i % 2 === 0 && i > 0) y += Math.max(150, 92 + Math.max(models[i - 1].ports.length, models[i - 2].ports.length) * 26) + 60;
    node.position = { x, y, height };
    for (const [p, port] of node.ports.entries()) positions.set(port.identity, { x: x + (port.direction === 'input' ? 0 : 320), y: y + 80 + p * 26 });
  }
  const height = Math.max(240, ...models.map(node => node.position.y + node.position.height + 32));
  svg.setAttribute('viewBox', `0 0 820 ${height}`); svg.style.width = '100%'; svg.style.minWidth = '640px';
  const edges = create('g', {});
  for (const cord of graph.cords) {
    const a = positions.get(cord.source), b = positions.get(cord.sink);
    const bend = Math.max(45, Math.abs(b.x - a.x) / 2);
    const path = create('path', { d: `M ${a.x} ${a.y} C ${a.x + bend} ${a.y}, ${b.x - bend} ${b.y}, ${b.x} ${b.y}`, fill: 'none', stroke: cord.identity === graph.selected ? 'var(--conduit-emphasis, #76ddb0)' : 'var(--conduit-text-secondary, #91aaa0)', 'stroke-width': cord.identity === graph.selected ? 5 : 3 }, edges);
    selectable(path, cord.identity, `Cord from ${graph.ports.find(p => p.identity === cord.source).label} to ${graph.ports.find(p => p.identity === cord.sink).label}`);
  }
  for (const node of models) {
    const { x, y, height } = node.position;
    const group = create('g', {});
    create('rect', { x, y, width: 320, height, rx: 10, fill: 'var(--conduit-surface, #17241e)', stroke: node.identity === graph.selected ? 'var(--conduit-emphasis, #76ddb0)' : 'var(--conduit-structure-secondary, #456055)', 'stroke-width': 2 }, group);
    const heading = create('text', { x: x + 16, y: y + 28, fill: 'var(--conduit-text-primary, #ecf7ee)', 'font-size': 17 }, group);
    heading.textContent = node.label.length > 28 ? `${node.label.slice(0, 25)}…` : node.label;
    create('title', {}, heading).textContent = node.label;
    const kind = create('text', { x: x + 16, y: y + 50, fill: 'var(--conduit-text-secondary, #91aaa0)', 'font-size': 12 }, group); kind.textContent = node.kind;
    if (node.identity) selectable(heading, node.identity, `Gear ${node.label}, ${node.kind}`);
    for (const port of node.ports) {
      const point = positions.get(port.identity);
      const row = create('g', {}, group);
      create('circle', { cx: point.x, cy: point.y, r: 6, fill: port.identity === graph.selected ? 'var(--conduit-emphasis, #76ddb0)' : 'var(--conduit-text-primary, #ecf7ee)' }, row);
      const text = create('text', { x: x + 16, y: point.y + 4, fill: 'var(--conduit-text-primary, #ecf7ee)', 'font-size': 12 }, row);
      text.textContent = `${port.direction === 'input' ? '←' : '→'} ${port.label} · ${port.info}`;
      selectable(row, port.identity, `${port.direction} ${port.label}, ${port.info}, ${port.temporal}`);
    }
  }
  element.append(svg);
}
