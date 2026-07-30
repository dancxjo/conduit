/**
 * Presentation-only smart cord routing for Patchbay.
 *
 * The route is computed from React Flow's measured node rectangles. Conduit
 * source, plans, and evidence remain untouched: this module only chooses the
 * SVG path used to draw a projected cord.
 */

const e = window.React.createElement;
const GRID = 16;
const NODE_CLEARANCE = 16;
const SEARCH_MARGIN = 96;
const MAX_SEARCH_CELLS = 24_000;
const CORNER_RADIUS = 10;

function cellKey(x, y) {
  return `${x},${y}`;
}

function pointForCell(cell, origin) {
  return {
    x: origin.x + cell.x * GRID,
    y: origin.y + cell.y * GRID,
  };
}

function simplify(points) {
  if (points.length < 3) return points;
  const result = [points[0]];
  for (let index = 1; index < points.length - 1; index += 1) {
    const previous = result[result.length - 1];
    const current = points[index];
    const next = points[index + 1];
    const vertical = previous.x === current.x && current.x === next.x;
    const horizontal = previous.y === current.y && current.y === next.y;
    if (!vertical && !horizontal) result.push(current);
  }
  result.push(points[points.length - 1]);
  return result;
}

function pointBeforeCorner(previous, corner, radius) {
  const distance = Math.hypot(corner.x - previous.x, corner.y - previous.y);
  const scale = Math.min(radius, distance / 2) / distance;
  return {
    x: corner.x + (previous.x - corner.x) * scale,
    y: corner.y + (previous.y - corner.y) * scale,
  };
}

function svgSmoothStepPath(points) {
  if (points.length < 2) return "";
  if (points.length === 2) {
    return `M ${points[0].x} ${points[0].y} L ${points[1].x} ${points[1].y}`;
  }

  const commands = [`M ${points[0].x} ${points[0].y}`];
  for (let index = 1; index < points.length - 1; index += 1) {
    const previous = points[index - 1];
    const corner = points[index];
    const next = points[index + 1];
    const intoCorner = pointBeforeCorner(previous, corner, CORNER_RADIUS);
    const outOfCorner = pointBeforeCorner(next, corner, CORNER_RADIUS);
    commands.push(`L ${intoCorner.x} ${intoCorner.y}`);
    commands.push(`Q ${corner.x} ${corner.y} ${outOfCorner.x} ${outOfCorner.y}`);
  }
  const last = points[points.length - 1];
  commands.push(`L ${last.x} ${last.y}`);
  return commands.join(" ");
}

function nodeRectangle(node) {
  const position = node.positionAbsolute || node.position || { x: 0, y: 0 };
  const width = node.width || (node.__rf && node.__rf.width) || 0;
  const height = node.height || (node.__rf && node.__rf.height) || 0;
  if (!width || !height) return null;
  return {
    left: position.x - NODE_CLEARANCE,
    top: position.y - NODE_CLEARANCE,
    right: position.x + width + NODE_CLEARANCE,
    bottom: position.y + height + NODE_CLEARANCE,
  };
}

/**
 * Computes a bounded orthogonal A* route around measured node rectangles.
 * Returns null when a bounded route cannot be found so rendering can fall
 * back to React Flow's built-in smooth-step edge.
 */
export function routeAroundNodes(source, target, nodes, endpointNodeIds = []) {
  const obstacles = nodes
    .filter((node) => !endpointNodeIds.includes(node.id))
    .map(nodeRectangle)
    .filter(Boolean);

  const left = Math.min(source.x, target.x, ...obstacles.map((rect) => rect.left)) - SEARCH_MARGIN;
  const top = Math.min(source.y, target.y, ...obstacles.map((rect) => rect.top)) - SEARCH_MARGIN;
  const right = Math.max(source.x, target.x, ...obstacles.map((rect) => rect.right)) + SEARCH_MARGIN;
  const bottom = Math.max(source.y, target.y, ...obstacles.map((rect) => rect.bottom)) + SEARCH_MARGIN;
  const origin = {
    x: Math.floor(left / GRID) * GRID,
    y: Math.floor(top / GRID) * GRID,
  };
  const columns = Math.ceil((right - origin.x) / GRID) + 1;
  const rows = Math.ceil((bottom - origin.y) / GRID) + 1;
  if (columns * rows > MAX_SEARCH_CELLS) return null;

  const toCell = (point) => ({
    x: Math.round((point.x - origin.x) / GRID),
    y: Math.round((point.y - origin.y) / GRID),
  });
  const start = toCell(source);
  const goal = toCell(target);
  const blocked = (cell) => {
    if ((cell.x === start.x && cell.y === start.y) ||
        (cell.x === goal.x && cell.y === goal.y)) return false;
    const point = pointForCell(cell, origin);
    return obstacles.some((rect) =>
      point.x >= rect.left && point.x <= rect.right &&
      point.y >= rect.top && point.y <= rect.bottom
    );
  };

  const open = [{ ...start, score: 0 }];
  const cameFrom = new Map();
  const cost = new Map([[cellKey(start.x, start.y), 0]]);
  const visited = new Set();
  const directions = [[1, 0], [0, 1], [-1, 0], [0, -1]];

  while (open.length) {
    open.sort((a, b) => a.score - b.score);
    const current = open.shift();
    const currentKey = cellKey(current.x, current.y);
    if (visited.has(currentKey)) continue;
    visited.add(currentKey);

    if (current.x === goal.x && current.y === goal.y) {
      const cells = [goal];
      let cursorKey = currentKey;
      while (cameFrom.has(cursorKey)) {
        const previous = cameFrom.get(cursorKey);
        cells.push(previous);
        cursorKey = cellKey(previous.x, previous.y);
      }
      cells.reverse();
      const points = cells.map((cell) => pointForCell(cell, origin));
      points[0] = source;
      points[points.length - 1] = target;
      const simplified = simplify(points);
      return {
        path: svgSmoothStepPath(simplified),
        points: simplified,
      };
    }

    for (const [dx, dy] of directions) {
      const next = { x: current.x + dx, y: current.y + dy };
      if (next.x < 0 || next.y < 0 || next.x >= columns || next.y >= rows || blocked(next)) {
        continue;
      }
      const nextKey = cellKey(next.x, next.y);
      const nextCost = cost.get(currentKey) + 1;
      if (nextCost >= (cost.get(nextKey) ?? Infinity)) continue;
      cost.set(nextKey, nextCost);
      cameFrom.set(nextKey, { x: current.x, y: current.y });
      const heuristic = Math.abs(goal.x - next.x) + Math.abs(goal.y - next.y);
      open.push({ ...next, score: nextCost + heuristic });
    }
  }
  return null;
}

export function PatchbaySmartEdge(props) {
  const nodes = window.ReactFlow.useNodes();
  const routed = routeAroundNodes(
    { x: props.sourceX, y: props.sourceY },
    { x: props.targetX, y: props.targetY },
    nodes,
    [props.source, props.target],
  );

  if (!routed) {
    return e(window.ReactFlow.SmoothStepEdge, props);
  }

  const middle = routed.points[Math.floor(routed.points.length / 2)];
  return e(window.React.Fragment, null,
    e("path", {
      id: props.id,
      className: `react-flow__edge-path patchbay-smart-cord ${props.data?.presentationClass || ""}`,
      d: routed.path,
      style: props.style,
      markerEnd: props.markerEnd,
      markerStart: props.markerStart,
      fill: "none",
    }),
    props.label ? e(window.ReactFlow.EdgeText, {
      x: middle.x,
      y: middle.y,
      label: props.label,
      labelStyle: props.labelStyle,
      labelBgStyle: props.labelBgStyle,
      labelBgPadding: props.labelBgPadding,
    }) : null,
  );
}
