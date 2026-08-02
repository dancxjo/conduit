/**
 * Presentation-only straight cord routing for Patchbay.
 *
 * Every measured faceplate is an obstacle. One rectilinear channel graph is
 * built from those rectangles, and one shortest-path search chooses the cord.
 * No curve or unsafe built-in edge is used as a fallback.
 */

const e = window.React.createElement;
const NODE_CLEARANCE = 24;
const CHANNEL_GAP = 4;
const BEND_COST = 32;

function nodeRectangle(node) {
  const position = node.positionAbsolute || node.position || { x: 0, y: 0 };
  const width = node.width || node.measured?.width ||
    node.__rf?.width || Number.parseFloat(node.style?.width) || 0;
  const height = node.height || node.measured?.height ||
    node.__rf?.height || Number.parseFloat(node.style?.height) || 0;
  if (!width || !height) return null;
  return {
    left: position.x,
    top: position.y,
    right: position.x + width,
    bottom: position.y + height,
  };
}

function inflate(rect, margin) {
  return {
    left: rect.left - margin,
    top: rect.top - margin,
    right: rect.right + margin,
    bottom: rect.bottom + margin,
  };
}

function pointInside(rect, point) {
  return point.x > rect.left && point.x < rect.right &&
    point.y > rect.top && point.y < rect.bottom;
}

function segmentCrosses(rect, from, to) {
  if (from.x === to.x) {
    return from.x > rect.left && from.x < rect.right &&
      Math.max(from.y, to.y) > rect.top &&
      Math.min(from.y, to.y) < rect.bottom;
  }
  if (from.y === to.y) {
    return from.y > rect.top && from.y < rect.bottom &&
      Math.max(from.x, to.x) > rect.left &&
      Math.min(from.x, to.x) < rect.right;
  }
  return true;
}

function routeDirection(position, point, rect) {
  const named = String(position || "").toLowerCase();
  if (["left", "right", "top", "bottom"].includes(named)) return named;
  return [
    ["left", Math.abs(point.x - rect.left)],
    ["right", Math.abs(point.x - rect.right)],
    ["top", Math.abs(point.y - rect.top)],
    ["bottom", Math.abs(point.y - rect.bottom)],
  ].sort((a, b) => a[1] - b[1])[0][0];
}

function escapePoint(point, rect, position) {
  const obstacle = inflate(rect, NODE_CLEARANCE);
  const direction = routeDirection(position, point, rect);
  if (direction === "left") {
    return { x: obstacle.left - CHANNEL_GAP, y: point.y };
  }
  if (direction === "right") {
    return { x: obstacle.right + CHANNEL_GAP, y: point.y };
  }
  if (direction === "top") {
    return { x: point.x, y: obstacle.top - CHANNEL_GAP };
  }
  return { x: point.x, y: obstacle.bottom + CHANNEL_GAP };
}

function pointKey(point) {
  return `${point.x},${point.y}`;
}

function uniqueSorted(values) {
  return [...new Set(values)].sort((a, b) => a - b);
}

function addNeighbor(graph, from, to) {
  const distance = Math.abs(to.x - from.x) + Math.abs(to.y - from.y);
  if (distance === 0) return;
  const direction = from.x === to.x ? "vertical" : "horizontal";
  graph.get(pointKey(from)).push({ point: to, distance, direction });
  graph.get(pointKey(to)).push({ point: from, distance, direction });
}

function channelGraph(start, goal, obstacles) {
  const xCoordinates = uniqueSorted([
    start.x,
    goal.x,
    ...obstacles.flatMap((rect) => [
      rect.left - CHANNEL_GAP,
      rect.right + CHANNEL_GAP,
    ]),
  ]);
  const yCoordinates = uniqueSorted([
    start.y,
    goal.y,
    ...obstacles.flatMap((rect) => [
      rect.top - CHANNEL_GAP,
      rect.bottom + CHANNEL_GAP,
    ]),
  ]);
  const graph = new Map();
  const rows = new Map(yCoordinates.map((y) => [y, []]));
  const columns = new Map(xCoordinates.map((x) => [x, []]));

  for (const y of yCoordinates) {
    for (const x of xCoordinates) {
      const point = { x, y };
      if (obstacles.some((rect) => pointInside(rect, point))) continue;
      graph.set(pointKey(point), []);
      rows.get(y).push(point);
      columns.get(x).push(point);
    }
  }

  const connectVisibleNeighbors = (lines, coordinate) => {
    for (const points of lines.values()) {
      points.sort((a, b) => a[coordinate] - b[coordinate]);
      for (let index = 1; index < points.length; index += 1) {
        const from = points[index - 1];
        const to = points[index];
        if (!obstacles.some((rect) => segmentCrosses(rect, from, to))) {
          addNeighbor(graph, from, to);
        }
      }
    }
  };
  connectVisibleNeighbors(rows, "x");
  connectVisibleNeighbors(columns, "y");
  return graph;
}

function shortestRoute(start, goal, obstacles) {
  const graph = channelGraph(start, goal, obstacles);
  if (!graph.has(pointKey(start)) || !graph.has(pointKey(goal))) return null;

  const startState = `${pointKey(start)}|start`;
  const open = [{ point: start, direction: "start", cost: 0, score: 0 }];
  const costs = new Map([[startState, 0]]);
  const previous = new Map();
  let goalState = null;

  while (open.length > 0) {
    open.sort((a, b) => a.score - b.score || a.cost - b.cost ||
      pointKey(a.point).localeCompare(pointKey(b.point)));
    const current = open.shift();
    const currentState = `${pointKey(current.point)}|${current.direction}`;
    if (current.cost !== costs.get(currentState)) continue;
    if (pointKey(current.point) === pointKey(goal)) {
      goalState = currentState;
      break;
    }

    for (const neighbor of graph.get(pointKey(current.point))) {
      const bend = current.direction !== "start" &&
        current.direction !== neighbor.direction;
      const cost = current.cost + neighbor.distance + (bend ? BEND_COST : 0);
      const state = `${pointKey(neighbor.point)}|${neighbor.direction}`;
      if (cost >= (costs.get(state) ?? Infinity)) continue;
      costs.set(state, cost);
      previous.set(state, currentState);
      open.push({
        point: neighbor.point,
        direction: neighbor.direction,
        cost,
        score: cost + Math.abs(goal.x - neighbor.point.x) +
          Math.abs(goal.y - neighbor.point.y),
      });
    }
  }
  if (!goalState) return null;

  const points = [];
  for (let state = goalState; state; state = previous.get(state)) {
    const [coordinates] = state.split("|");
    const [x, y] = coordinates.split(",").map(Number);
    points.push({ x, y });
  }
  return points.reverse();
}

function simplify(points) {
  const result = [];
  for (const point of points) {
    const previous = result[result.length - 1];
    if (previous && previous.x === point.x && previous.y === point.y) continue;
    const beforePrevious = result[result.length - 2];
    if (beforePrevious && previous &&
        ((beforePrevious.x === previous.x && previous.x === point.x) ||
         (beforePrevious.y === previous.y && previous.y === point.y))) {
      result[result.length - 1] = point;
    } else {
      result.push(point);
    }
  }
  return result;
}

function svgPath(points) {
  return points.map((point, index) =>
    `${index === 0 ? "M" : "L"} ${point.x} ${point.y}`
  ).join(" ");
}

function boxesOverlap(a, b) {
  return a.left < b.right && a.right > b.left &&
    a.top < b.bottom && a.bottom > b.top;
}

function chooseLabelPoint(points, nodeRects, label) {
  const halfWidth = Math.min(220, 12 + String(label).length * 3.2);
  const halfHeight = 14;
  let best = null;
  for (let index = 1; index < points.length; index += 1) {
    const from = points[index - 1];
    const to = points[index];
    const length = Math.abs(to.x - from.x) + Math.abs(to.y - from.y);
    for (const ratio of [0.5, 0.25, 0.75]) {
      const point = {
        x: from.x + (to.x - from.x) * ratio,
        y: from.y + (to.y - from.y) * ratio,
      };
      const labelBox = {
        left: point.x - halfWidth,
        right: point.x + halfWidth,
        top: point.y - halfHeight,
        bottom: point.y + halfHeight,
      };
      if (nodeRects.some((rect) => boxesOverlap(labelBox, inflate(rect, 6)))) {
        continue;
      }
      const score = length - Math.abs(0.5 - ratio) * 20;
      if (!best || score > best.score) best = { point, score };
    }
  }
  return best?.point || points[Math.floor(points.length / 2)];
}

export function routeStraightCord(source, target, nodes, endpointIds, positions) {
  const rectangles = nodes.map(nodeRectangle);
  if (rectangles.some((rect) => rect === null)) return null;
  const sourceIndex = nodes.findIndex((node) => node.id === endpointIds[0]);
  const targetIndex = nodes.findIndex((node) => node.id === endpointIds[1]);
  if (sourceIndex < 0 || targetIndex < 0) return null;

  const obstacles = rectangles.map((rect) => inflate(rect, NODE_CLEARANCE));
  const start = escapePoint(source, rectangles[sourceIndex], positions.source);
  const goal = escapePoint(target, rectangles[targetIndex], positions.target);
  const sourceBlocked = obstacles.some((rect, index) =>
    index !== sourceIndex && segmentCrosses(rect, source, start));
  const targetBlocked = obstacles.some((rect, index) =>
    index !== targetIndex && segmentCrosses(rect, goal, target));
  if (sourceBlocked || targetBlocked) return null;

  const middle = shortestRoute(start, goal, obstacles);
  return middle ? simplify([source, ...middle, target]) : null;
}

export function PatchbayCordEdge(props) {
  const nodes = window.ReactFlow.useNodes();
  const points = routeStraightCord(
    { x: props.sourceX, y: props.sourceY },
    { x: props.targetX, y: props.targetY },
    nodes,
    [props.source, props.target],
    { source: props.sourcePosition, target: props.targetPosition },
  );
  if (!points) return null;

  const labelPoint = chooseLabelPoint(
    points,
    nodes.map(nodeRectangle).filter(Boolean),
    props.label,
  );
  const path = svgPath(points);
  return e(window.React.Fragment, null,
    e("path", {
      className: "react-flow__edge-interaction",
      d: path,
      fill: "none",
      stroke: "transparent",
      strokeWidth: 20,
      style: { pointerEvents: "stroke" },
    }),
    e("path", {
      id: props.id,
      className: "react-flow__edge-path",
      d: path,
      style: props.style,
      markerEnd: props.markerEnd,
      markerStart: props.markerStart,
      "data-source-node": props.source,
      "data-target-node": props.target,
      "data-routing-mode": "rectilinear",
      "data-cord-geometry": "straight",
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
