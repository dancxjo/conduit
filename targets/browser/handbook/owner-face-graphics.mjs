// Browser realization of finite GraphicsScene commands prepared by Wasm. This
// leaf owns no acquisition, clock alignment, interpolation, storage or authority.
const NS = 'http://www.w3.org/2000/svg';
const paints = Object.freeze({ Background: 'var(--face-paper)', Foreground: 'var(--face-ink)',
  Accent: 'var(--face-accent)', Status: 'var(--face-muted)', Muted: 'var(--face-muted)',
  Success: 'var(--look-success,#39765a)', Warning: 'var(--look-warning,#987022)',
  Danger: 'var(--look-failure,#b34b45)', Focus: 'var(--face-focus)', Hovered: 'var(--face-card)',
  Selected: 'var(--face-accent)', Surface: 'var(--face-card)', Border: 'var(--face-line)' });
const roles = new Set(['Body','Title','Heading','Label','Code','Status','Warning','Action','Muted']);
const styles = new Set(['Fill','Stroke','RoundedFill','RoundedStroke']);
let sequence = 0;
const fail = () => { throw new TypeError('Invalid bounded GraphicsScene view'); };
const rect = r => Array.isArray(r) && r.length === 4 && r.every(Number.isInteger)
  && r[0] >= -32768 && r[1] >= -32768 && r[2] > 0 && r[3] > 0
  && r[2] <= 32767 && r[3] <= 32767 && r[0]+r[2] <= 32767 && r[1]+r[3] <= 32767;
const node = (name, attrs = {}) => {
  const element = document.createElementNS(NS, name);
  for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, String(value));
  return element;
};
export function graphicsSceneElement(view, label) {
  if (typeof label !== 'string' || label.length > 192) fail();
  if (!view || !Number.isInteger(view.width) || !Number.isInteger(view.height)
    || view.width < 1 || view.height < 1 || view.width > 32767 || view.height > 32767
    || !Array.isArray(view.commands) || view.commands.length > 48) fail();
  if (view.refusal !== null && view.refusal !== undefined) {
    if (typeof view.refusal !== 'string' || view.refusal.length > 256 || view.commands.length) fail();
    const refusal = document.createElement('p'); refusal.textContent = view.refusal;
    refusal.setAttribute('role','status'); return refusal;
  }
  const svg = node('svg', { viewBox: `0 0 ${view.width} ${view.height}`, role: 'img',
    'aria-label': label, preserveAspectRatio: 'xMidYMid meet' });
  svg.style.width = '100%'; svg.style.minWidth = `${view.width}px`;
  svg.style.height = 'auto'; svg.style.display = 'block';
  const defs = node('defs'); svg.append(defs);
  const prefix = `owner-graphics-${++sequence}-`;
  for (const [index, command] of view.commands.entries()) {
    if (!command || !rect(command.bounds) || !rect(command.clip) || !Object.hasOwn(paints,command.paint)
      || !styles.has(command.style) || !roles.has(command.text_role)
      || typeof command.text !== 'string' || new TextEncoder().encode(command.text).length > 192
      || !Array.isArray(command.points) || command.points.length > 16) fail();
    const [x,y,width,height] = command.bounds;
    const clipId = prefix+index;
    const clip = node('clipPath', { id: clipId, clipPathUnits: 'userSpaceOnUse' });
    const [cx,cy,cw,ch] = command.clip;
    clip.append(node('rect',{x:cx,y:cy,width:cw,height:ch})); defs.append(clip);
    const stroke = command.style.endsWith('Stroke');
    let shape;
    if (command.kind === 'Rect') {
      if (command.text || command.points.length) fail();
      shape = node('rect', {x,y,width,height,rx:command.style.startsWith('Rounded') ? 4 : 0});
    } else if (command.kind === 'Text') {
      if (stroke || !command.text || command.points.length) fail();
      shape = node('text', { x, y: y+2, 'dominant-baseline': 'hanging',
        'font-family': command.text_role === 'Code' ? 'ui-monospace,monospace' : 'inherit' });
      shape.style.fontSize = 'var(--look-type-small,16px)';
      shape.dataset.textPurpose = command.text_role;
      shape.textContent = command.text;
    } else if (command.kind === 'Polyline' || command.kind === 'OrthogonalPath') {
      if (!stroke || command.text || command.points.length < 2
        || command.kind === 'OrthogonalPath' && command.points.length > 8
        || command.points.some(p => !Array.isArray(p) || p.length !== 2
          || p.some(v => !Number.isInteger(v) || v < -32768 || v > 32767))) fail();
      shape = node('polyline', { points:command.points.map(p => p.join(',')).join(' '), 'stroke-width':2 });
    } else fail(); // Unsupported leaves refuse; no text/URL/icon fallbacks.
    shape.setAttribute('fill', stroke ? 'none' : paints[command.paint]);
    if (stroke) shape.setAttribute('stroke', paints[command.paint]);
    shape.setAttribute('clip-path', `url(#${clipId})`);
    svg.append(shape);
  }
  // Preserve admitted label sizes on narrow surfaces. The native graphical
  // viewport scrolls instead of squeezing a complete chart into unreadable type.
  const viewport = document.createElement('div');
  viewport.style.overflowX = 'auto'; viewport.style.maxWidth = '100%';
  viewport.setAttribute('role', 'region'); viewport.setAttribute('aria-label', label);
  viewport.tabIndex = 0; viewport.append(svg);
  return viewport;
}
