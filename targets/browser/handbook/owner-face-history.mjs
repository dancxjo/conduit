// Drawing material is prepared from the checked Face in Rust. This adapter
// retains no readings, clock, history, interactions or acknowledgement state.
const RECIPE = 'conduit.face/environment-history@1';
const node = (tag, text) => {
  const element = document.createElement(tag);
  element.textContent = text;
  return element;
};
const text = value => typeof value === 'string' && value.length <= 256;
const count = value => text(value) && /^(0|[1-9][0-9]{0,19})$/.test(value);
const quantity = value => value === null || (text(value) && /^-?[0-9]{1,20}\.[0-9]{3}$/.test(value));
const coordinate = (value, min, max) => Number.isInteger(value) && value >= min && value <= max;
const pair = (value, y, min, max) => (value === null && y === null)
  || (value !== null && coordinate(y, min, max));

function valid(view, graph) {
  return graph?.schema === 'conduit.browser/environment-history@1'
    && text(graph.face_id) && graph.face_id === view.face_id
    && count(graph.face_revision) && graph.face_revision === String(view.face_revision)
    && graph.subject === 'environment-history'
    && view.subjects.some(subject => subject.identity === graph.subject)
    && Array.isArray(graph.producer) && graph.producer.length === 4 && graph.producer.every(text)
    && count(graph.omitted) && quantity(graph.temperature_top) && graph.temperature_top !== null
    && quantity(graph.temperature_bottom) && graph.temperature_bottom !== null
    && Array.isArray(graph.samples) && graph.samples.length <= 16
    && graph.samples.every(sample => sample !== null && typeof sample === 'object'
      && count(sample.sequence) && count(sample.boot_ms)
      && quantity(sample.temperature) && quantity(sample.humidity)
      && coordinate(sample.x, 32, 592)
      && pair(sample.temperature, sample.temperature_y, 40, 140)
      && pair(sample.humidity, sample.humidity_y, 192, 292)
      && (sample.failure === null || (text(sample.failure)
        && sample.temperature === null && sample.humidity === null))
      && (sample.gap_before === null || (Array.isArray(sample.gap_before)
        && sample.gap_before.length === 2 && sample.gap_before.every(count))));
}

export function historyRegion(view) {
  const component = view.component;
  if (component?.recipe_identity !== RECIPE) return null;
  if (component.fallback) return node('p', 'History graph unavailable; inspect the recorded Face values below.');
  const graph = component.history;
  if (!valid(view, graph)) return node('p', 'History graph unavailable; inspect the Face values below.');
  const region = node('section', '');
  region.className = 'owner-face-history';
  region.setAttribute('part', 'history');
  region.dataset.faceId = graph.face_id;
  region.dataset.faceRevision = graph.face_revision;
  region.append(node('h3', 'Recorded temperature and humidity'));
  region.append(node('p', `Recovered history; ${graph.omitted} samples outside this view. Earlier history is not known.`));
  region.append(node('p', `Producer Boot: ${graph.producer[1]}. Times are milliseconds within that Boot.`));
  if (!graph.samples.length) {
    region.append(node('p', 'No recovered samples in this view.'));
    return region;
  }
  region.append(node('p', `Temperature °C: top ${graph.temperature_top}; bottom ${graph.temperature_bottom}. Humidity %: top 100; bottom 0.`));
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 640 320');
  svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', 'Temperature above and humidity below, by producer Boot time. Exact values and missing samples follow in the table.');
  for (const [top, bottom] of [[40, 140], [192, 292]]) {
    const axes = document.createElementNS(svg.namespaceURI, 'path');
    axes.setAttribute('d', `M32 ${top}V${bottom}H592`);
    axes.setAttribute('fill', 'none');
    axes.setAttribute('stroke', 'currentColor');
    svg.append(axes);
  }
  for (const sample of graph.samples) {
    for (const y of [sample.temperature_y, sample.humidity_y]) {
      if (y === null) continue;
      const point = document.createElementNS(svg.namespaceURI, 'circle');
      point.setAttribute('cx', String(sample.x));
      point.setAttribute('cy', String(y));
      point.setAttribute('r', '3');
      point.setAttribute('fill', 'currentColor');
      svg.append(point);
    }
  }
  region.append(svg);
  region.append(node('p', `Boot milliseconds: left ${graph.samples[0].boot_ms}; right ${graph.samples.at(-1).boot_ms}. Dots are observations; gaps are not connected.`));
  const details = node('details', '');
  details.append(node('summary', 'Exact recorded values and gaps'));
  const scroll = node('div', '');
  scroll.className = 'owner-face-history-table';
  scroll.tabIndex = 0;
  scroll.setAttribute('role', 'region');
  scroll.setAttribute('aria-label', 'Recorded samples table');
  const table = node('table', '');
  table.append(node('caption', 'Recovered samples; unavailable readings stay absent'));
  const header = node('tr', '');
  for (const title of ['Sequence', 'Boot ms', 'Temperature °C', 'Humidity %', 'Status']) {
    const heading = node('th', title);
    heading.scope = 'col';
    header.append(heading);
  }
  const head = node('thead', ''); head.append(header); table.append(head);
  const body = node('tbody', '');
  for (const sample of graph.samples) {
    const row = node('tr', '');
    const gap = sample.gap_before ? `; missing sequences ${sample.gap_before[0]} through ${sample.gap_before[1]}` : '';
    for (const value of [sample.sequence, sample.boot_ms, sample.temperature ?? 'Unavailable',
      sample.humidity ?? 'Unavailable', `${sample.failure ?? 'Recorded'}${gap}`]) row.append(node('td', value));
    body.append(row);
  }
  table.append(body); scroll.append(table); details.append(scroll); region.append(details);
  return region;
}
