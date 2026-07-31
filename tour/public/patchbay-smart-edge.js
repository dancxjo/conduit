/**
 * Presentation-only smart cord routing for Patchbay.
 *
 * The route is computed from React Flow's measured node rectangles. Conduit
 * source, plans, and evidence remain untouched: this module only chooses the
 * SVG path used to draw a projected cord.
 */

const e = window.React.createElement;
const GRID = 16;
const NODE_CLEARANCE = 14;
const LABEL_CLEARANCE = 16;
const SEARCH_MARGIN = 96;
const MAX_SEARCH_CELLS = 24_000;
const FLOWINESS = 0.32;
const PATH_SAMPLE_COUNT = 12;
const SAMPLE_POINTS_PER_CELL = 4;

function cellKey(x, y) {
  return `${x},${y}`;
}

function inflatedRect(rect, margin) {
  return {
    left: rect.left - margin,
    top: rect.top - margin,
    right: rect.right + margin,
    bottom: rect.bottom + margin,
  };
}

function pointInRect(point, rect, inclusive = false) {
  if (inclusive) {
    return point.x >= rect.left &&
      point.x <= rect.right &&
      point.y >= rect.top &&
      point.y <= rect.bottom;
  }
  return point.x > rect.left &&
    point.x < rect.right &&
    point.y > rect.top &&
    point.y < rect.bottom;
}

function pointForCell(cell, origin) {
  return {
    x: origin.x + cell.x * GRID,
    y: origin.y + cell.y * GRID,
  };
}

function cellArea(point) {
  const half = GRID / 2;
  return {
    left: point.x - half,
    right: point.x + half,
    top: point.y - half,
    bottom: point.y + half,
  };
}

function rectsOverlap(a, b) {
  return a.left <= b.right &&
    a.right >= b.left &&
    a.top <= b.bottom &&
    a.bottom >= b.top;
}

function segmentIntersectsRect(p1, p2, rect) {
  const dx = p2.x - p1.x;
  const dy = p2.y - p1.y;
  const distance = Math.max(Math.abs(dx), Math.abs(dy));
  const steps = Math.max(
    1,
    Math.ceil(distance / (GRID / SAMPLE_POINTS_PER_CELL)),
  );
  for (let step = 0; step <= steps; step += 1) {
    const ratio = step / steps;
    const point = {
      x: p1.x + dx * ratio,
      y: p1.y + dy * ratio,
    };
    if (pointInRect(point, rect, true)) {
      return true;
    }
  }
  return false;
}

function segmentHasCollision(p1, p2, obstacles) {
  return obstacles.some((obstacle) => segmentIntersectsRect(p1, p2, obstacle));
}

function segmentsFromPoints(points) {
  const segments = [];
  for (let index = 0; index < points.length - 1; index += 1) {
    segments.push({ from: points[index], to: points[index + 1] });
  }
  return segments;
}

function pointDistanceToRect(point, rect) {
  const dx = point.x < rect.left
    ? rect.left - point.x
    : point.x > rect.right
      ? point.x - rect.right
      : 0;
  const dy = point.y < rect.top
    ? rect.top - point.y
    : point.y > rect.bottom
      ? point.y - rect.bottom
      : 0;
  return Math.sqrt(dx * dx + dy * dy);
}

function pointIsSafe(point, obstacles, clearance = LABEL_CLEARANCE) {
  if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) return false;
  return !obstacles.some((obstacle) =>
    pointInRect(point, inflatedRect(obstacle, clearance))
  );
}

function chooseLabelPoint(points, obstacles, clearance = LABEL_CLEARANCE) {
  let bestCandidate = null;
  const segments = segmentsFromPoints(points);
  for (const segment of segments) {
    const deltaX = segment.to.x - segment.from.x;
    const deltaY = segment.to.y - segment.from.y;
    const length = Math.max(Math.abs(deltaX), Math.abs(deltaY));
    if (length < 40) continue;
    const probes = 5;
    for (let probe = 1; probe < probes; probe += 1) {
      const ratio = probe / probes;
      const point = {
        x: segment.from.x + deltaX * ratio,
        y: segment.from.y + deltaY * ratio,
      };
      if (!pointIsSafe(point, obstacles, clearance)) continue;
      const distance = obstacles.length === 0
        ? Infinity
        : Math.min(...obstacles.map((rect) => pointDistanceToRect(point, rect)));
      if (!bestCandidate || distance > bestCandidate.distance) {
        bestCandidate = { point, distance };
      }
    }
  }
  return bestCandidate?.point ?? null;
}

function nudgeLabelOffNodes(point, obstacles) {
  const expanded = obstacles.map((rect) => ({
    left: rect.left - 96,
    right: rect.right + 96,
    top: rect.top - 18,
    bottom: rect.bottom + 18,
  }));
  let adjusted = { ...point };
  for (let iteration = 0; iteration < expanded.length * 2; iteration += 1) {
    const collision = expanded.find((rect) => pointInRect(adjusted, rect, true));
    if (!collision) break;
    const candidates = [
      { x: collision.left - 1, y: adjusted.y },
      { x: collision.right + 1, y: adjusted.y },
      { x: adjusted.x, y: collision.top - 1 },
      { x: adjusted.x, y: collision.bottom + 1 },
    ];
    candidates.sort((a, b) => {
      const collisionsA = expanded.filter((rect) => pointInRect(a, rect, true)).length;
      const collisionsB = expanded.filter((rect) => pointInRect(b, rect, true)).length;
      const distanceA = Math.abs(a.x - adjusted.x) + Math.abs(a.y - adjusted.y);
      const distanceB = Math.abs(b.x - adjusted.x) + Math.abs(b.y - adjusted.y);
      return collisionsA - collisionsB || distanceA - distanceB;
    });
    adjusted = candidates[0];
  }
  return adjusted;
}

function pointOnCubicBezier(t, p0, c1, c2, p1) {
  const oneMinusT = 1 - t;
  const a = oneMinusT ** 3;
  const b = 3 * oneMinusT ** 2 * t;
  const c = 3 * oneMinusT * t ** 2;
  const d = t ** 3;
  return {
    x: a * p0.x + b * c1.x + c * c2.x + d * p1.x,
    y: a * p0.y + b * c1.y + c * c2.y + d * p1.y,
  };
}

function pathHasCollision(points, obstacles) {
  if (points.length < 2) return false;
  if (points.length === 2) {
    return segmentHasCollision(points[0], points[1], obstacles);
  }
  for (let index = 0; index < points.length - 1; index += 1) {
    const p0 = points[index];
    const p3 = points[index + 1];
    const previous = points[Math.max(0, index - 1)];
    const next = points[Math.min(points.length - 1, index + 2)];
    const c1 = {
      x: p0.x + ((p3.x - previous.x) / 6) * FLOWINESS,
      y: p0.y + ((p3.y - previous.y) / 6) * FLOWINESS,
    };
    const c2 = {
      x: p3.x - ((next.x - p0.x) / 6) * FLOWINESS,
      y: p3.y - ((next.y - p0.y) / 6) * FLOWINESS,
    };
    const segmentLength = Math.max(Math.abs(p3.x - p0.x), Math.abs(p3.y - p0.y));
    const samples = Math.max(
      PATH_SAMPLE_COUNT,
      Math.ceil(segmentLength / (GRID / 2)),
    );
    for (let step = 0; step <= samples; step += 1) {
      const ratio = step / samples;
      const point = pointOnCubicBezier(ratio, p0, c1, c2, p3);
      if (obstacles.some((obstacle) => pointInRect(point, obstacle, true))) {
        return true;
      }
    }
  }
  return false;
}

function orthPathHasCollision(points, obstacles) {
  return segmentsFromPoints(points).some((segment) =>
    segmentHasCollision(segment.from, segment.to, obstacles),
  );
}

function orthPath(points) {
  if (points.length === 0) return "";
  return points.map((point, index) =>
    `${index === 0 ? "M" : "L"} ${point.x} ${point.y}`
  ).join(" ");
}

function routeLength(points) {
  let total = 0;
  for (let index = 0; index < points.length - 1; index += 1) {
    const current = points[index];
    const next = points[index + 1];
    total += Math.abs(next.x - current.x) + Math.abs(next.y - current.y);
  }
  return total;
}

/**
 * Produces a deterministic outer route around the full obstacle envelope when
 * grid-based A* cannot find a path within its bounded search window.
 */
function routeAroundBounds(source, target, obstacles) {
  const expandedBounds = obstacles.flatMap((rect) => [
    rect.left,
    rect.right,
    rect.top,
    rect.bottom,
  ]);
  const left = Math.min(source.x, target.x, ...expandedBounds) - SEARCH_MARGIN;
  const right = Math.max(source.x, target.x, ...expandedBounds) + SEARCH_MARGIN;
  const top = Math.min(source.y, target.y, ...expandedBounds) - SEARCH_MARGIN;
  const bottom = Math.max(source.y, target.y, ...expandedBounds) + SEARCH_MARGIN;

  const margin = SEARCH_MARGIN;
  const candidates = [];

  candidates.push([
    { x: source.x, y: source.y },
    { x: source.x, y: top - margin },
    { x: target.x, y: top - margin },
    { x: target.x, y: target.y },
  ]);
  candidates.push([
    { x: source.x, y: source.y },
    { x: source.x, y: bottom + margin },
    { x: target.x, y: bottom + margin },
    { x: target.x, y: target.y },
  ]);
  candidates.push([
    { x: source.x, y: source.y },
    { x: left - margin, y: source.y },
    { x: left - margin, y: target.y },
    { x: target.x, y: target.y },
  ]);
  candidates.push([
    { x: source.x, y: source.y },
    { x: right + margin, y: source.y },
    { x: right + margin, y: target.y },
    { x: target.x, y: target.y },
  ]);

  let best = null;
  for (const candidate of candidates) {
    const simplified = simplify(candidate);
    if (!orthPathHasCollision(simplified, obstacles)) {
      const length = routeLength(simplified);
      if (best === null || length < best.length) {
        best = {
          path: orthPath(simplified),
          points: simplified,
          length,
        };
      }
    }
  }

  if (!best) return null;
  const labelPoint = chooseLabelPoint(best.points, obstacles)
    ?? best.points[Math.floor(best.points.length / 2)];
  return {
    path: best.path,
    points: best.points,
    label: labelPoint,
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

function catmullRomToSmoothPath(points, tension = FLOWINESS) {
  if (points.length < 2) return "";
  if (points.length === 2) {
    return `M ${points[0].x} ${points[0].y} L ${points[1].x} ${points[1].y}`;
  }

  const commands = [`M ${points[0].x} ${points[0].y}`];
  for (let index = 0; index < points.length - 1; index += 1) {
    const previous = points[Math.max(0, index - 1)];
    const current = points[index];
    const next = points[index + 1];
    const nextNext = points[Math.min(points.length - 1, index + 2)];
    const cp1 = {
      x: current.x + ((next.x - previous.x) / 6) * tension,
      y: current.y + ((next.y - previous.y) / 6) * tension,
    };
    const cp2 = {
      x: next.x - ((nextNext.x - current.x) / 6) * tension,
      y: next.y - ((nextNext.y - current.y) / 6) * tension,
    };

    commands.push(
      `C ${cp1.x} ${cp1.y}, ${cp2.x} ${cp2.y}, ${next.x} ${next.y}`,
    );
  }
  return commands.join(" ");
}

function nodeRectangle(node) {
  const position = node.positionAbsolute || node.position || { x: 0, y: 0 };
  const width = node.width || node.measured?.width ||
    (node.__rf && node.__rf.width) || Number.parseFloat(node.style?.width) || 0;
  const height = node.height || node.measured?.height ||
    (node.__rf && node.__rf.height) || Number.parseFloat(node.style?.height) || 0;
  if (!width || !height) return null;
  return {
    left: position.x,
    top: position.y,
    right: position.x + width,
    bottom: position.y + height,
  };
}

function endpointEscapePoint(point, node, position) {
  if (!node) return point;
  const rect = nodeRectangle(node);
  if (!rect) return point;

  // React Flow places a handle on the faceplate edge. Start routing just
  // outside that faceplate so the endpoint node can remain an obstacle too.
  // This prevents a cord from leaving one port and later crossing behind a
  // different part of its own node.
  const direction = position || [
    ["left", Math.abs(point.x - rect.left)],
    ["right", Math.abs(point.x - rect.right)],
    ["top", Math.abs(point.y - rect.top)],
    ["bottom", Math.abs(point.y - rect.bottom)],
  ].sort((a, b) => a[1] - b[1])[0][0];
  const distance = NODE_CLEARANCE + GRID;
  if (direction === "left") return { x: rect.left - distance, y: point.y };
  if (direction === "right") return { x: rect.right + distance, y: point.y };
  if (direction === "top") return { x: point.x, y: rect.top - distance };
  return { x: point.x, y: rect.bottom + distance };
}

function endpointAwareRoute(routed, source, target) {
  if (!routed) return null;
  const points = simplify([source, ...routed.points, target]);
  const interiorPath = routed.path.replace(/^M\s+[^A-Za-z]+/, "");
  return {
    path: `M ${source.x} ${source.y} L ${routed.points[0].x} ${routed.points[0].y} ` +
      `${interiorPath} L ${target.x} ${target.y}`,
    points,
    label: routed.label,
  };
}

function routeAroundNodesWithMargin(source, target, nodes, endpointNodeIds, margin) {
  const obstacles = nodes
    .filter((node) => !endpointNodeIds.includes(node.id))
    .map(nodeRectangle)
    .filter(Boolean);

  const left = Math.min(source.x, target.x, ...obstacles.map((rect) => rect.left)) - margin;
  const top = Math.min(source.y, target.y, ...obstacles.map((rect) => rect.top)) - margin;
  const right = Math.max(source.x, target.x, ...obstacles.map((rect) => rect.right)) + margin;
  const bottom = Math.max(source.y, target.y, ...obstacles.map((rect) => rect.bottom)) + margin;
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
  const inflatedObstacles = obstacles
    .map((rect) => inflatedRect(rect, NODE_CLEARANCE));
  const blocked = (cell) => {
    if ((cell.x === start.x && cell.y === start.y) ||
        (cell.x === goal.x && cell.y === goal.y)) {
      return false;
    }
    const point = pointForCell(cell, origin);
    const area = cellArea(point);
    return inflatedObstacles.some((rect) => rectsOverlap(area, rect));
  };

  const open = [{ ...start, score: 0 }];
  const cameFrom = new Map();
  const cost = new Map([[cellKey(start.x, start.y), 0]]);
  const visited = new Set();
  const directions = [[1, 0], [0, 1], [-1, 0], [0, -1]];

  while (open.length) {
    open.sort((a, b) => a.score - b.score || a.y - b.y || a.x - b.x);
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
      const orthSafe = !orthPathHasCollision(simplified, inflatedObstacles);
      if (!orthSafe) return null;
      const smoothSafe = !pathHasCollision(simplified, inflatedObstacles);
      const path = smoothSafe
        ? catmullRomToSmoothPath(simplified)
        : orthPath(simplified);
      const labelPoint = chooseLabelPoint(simplified, inflatedObstacles)
        ?? simplified[Math.floor(simplified.length / 2)];
      if (!path) {
        return {
          path: orthPath(simplified),
          points: simplified,
          label: labelPoint,
        };
      }
      return {
        path,
        points: simplified,
        label: labelPoint,
      };
    }

    for (const [dx, dy] of directions) {
      const next = { x: current.x + dx, y: current.y + dy };
      if (next.x < 0 || next.y < 0 || next.x >= columns || next.y >= rows ||
          blocked(next)) {
        continue;
      }
      const nextPoint = pointForCell(next, origin);
      const currentPoint = pointForCell(current, origin);
      if (segmentHasCollision(currentPoint, nextPoint, inflatedObstacles)) continue;
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

/**
 * Computes a bounded orthogonal A* route around measured node rectangles.
 * Expands the search envelope in stages until a collision-free route is found.
 */
export function routeAroundNodes(
  source,
  target,
  nodes,
  endpointNodeIds = [],
  endpointPositions = {},
) {
  const sourceNode = nodes.find((node) => node.id === endpointNodeIds[0]);
  const targetNode = nodes.find((node) => node.id === endpointNodeIds[1]);
  const routeSource = endpointEscapePoint(
    source,
    sourceNode,
    endpointPositions.source,
  );
  const routeTarget = endpointEscapePoint(
    target,
    targetNode,
    endpointPositions.target,
  );
  for (let envelope = SEARCH_MARGIN; envelope <= SEARCH_MARGIN * 4; envelope += 64) {
    const routed = routeAroundNodesWithMargin(
      routeSource,
      routeTarget,
      nodes,
      [],
      envelope,
    );
    if (routed) {
      return endpointAwareRoute(routed, source, target);
    }
  }
  const fallback = routeAroundBounds(
    routeSource,
    routeTarget,
    nodes
      .map(nodeRectangle)
      .filter(Boolean)
      .map((rect) => inflatedRect(rect, NODE_CLEARANCE + SEARCH_MARGIN)),
  );
  if (fallback) {
    return endpointAwareRoute(fallback, source, target);
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
    { source: props.sourcePosition, target: props.targetPosition },
  );

  if (!routed) {
    return e(window.ReactFlow.StepEdge, props);
  }

  const labelObstacles = nodes.map(nodeRectangle).filter(Boolean);
  const chosenLabelPoint = chooseLabelPoint(routed.points, labelObstacles)
    || routed.label
    || routed.points[Math.floor(routed.points.length / 2)];
  const labelPoint = nudgeLabelOffNodes(chosenLabelPoint, labelObstacles);
  return e(window.React.Fragment, null,
    e("path", {
      id: props.id,
      className: "react-flow__edge-path",
      d: routed.path,
      style: props.style,
      markerEnd: props.markerEnd,
      markerStart: props.markerStart,
      "data-source-node": props.source,
      "data-target-node": props.target,
      fill: "none",
    }),
    props.label ? e(window.ReactFlow.EdgeText, {
      x: labelPoint.x,
      y: labelPoint.y,
      label: props.label,
      labelStyle: props.labelStyle,
      labelBgStyle: props.labelBgStyle,
      labelBgPadding: props.labelBgPadding,
      labelBgBorderRadius: props.labelBgBorderRadius,
    }) : null,
  );
}
