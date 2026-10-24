// Internal preparation. One vocabulary supplies both browser and native Masks.
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
const root = new URL('../../../', import.meta.url);
const data = JSON.parse(await readFile(new URL('gui-vocabulary.json', import.meta.url)));
if (data.schema !== 'conduit.gui/lucide-vocabulary@1') throw new Error('Unknown vocabulary schema');
const keys = Object.keys(data.icons);
const enumSource = await readFile(new URL('semantics/face/src/composition.rs', root), 'utf8');
const enumKeys = [...enumSource.match(/pub enum FaceIconKey \{([\s\S]*?)\}/)[1].matchAll(/\b([A-Z][A-Za-z0-9]*)\s*,/g)].map(match => match[1]);
if (JSON.stringify(keys) !== JSON.stringify(enumKeys)) throw new Error('Vocabulary must match every native icon key in order');
for (const value of [...Object.values(data.roles), ...Object.values(data.intents)]) {
  if (!Object.hasOwn(data.icons, value)) throw new Error(`Unknown mapped icon ${value}`);
}
const geometry = {};
for (const [key, name] of Object.entries(data.icons)) {
  if (!/^[A-Z][A-Za-z0-9]*$/.test(key) || !/^[a-z0-9-]+$/.test(name)) throw new Error('Invalid icon identity');
  const svg = await readFile(new URL(`svg/${name}.svg`, import.meta.url), 'utf8');
  if (!/viewBox="0 0 24 24"/.test(svg) || /<(script|foreignObject|image|use|style|text|animate|set|filter|a)\b/i.test(svg)) throw new Error(`Unsupported SVG ${name}`);
  geometry[key] = svg.slice(svg.indexOf('>') + 1, svg.lastIndexOf('</svg>')).trim();
}
const rust = source => {
  const result = spawnSync('rustfmt', ['+stable', '--edition', '2024', '--emit', 'stdout'], { input: source, encoding: 'utf8' });
  if (result.status !== 0) throw new Error(result.stderr);
  return result.stdout;
};
const header = '// Generated from mechanisms/implementations/bounded-lucide/gui-vocabulary.json.\n// Lucide notices: mechanisms/implementations/bounded-lucide/LICENSE.\n';
const native = rust(`${header}use conduit_face::{FaceIconKey as Icon, FaceRole};
pub(in crate::native_face_scene) fn for_role(role: &FaceRole) -> Option<Icon> { Some(match role {
${Object.entries(data.roles).map(([role,key])=>`FaceRole::${role} => Icon::${key},`).join('\n')}
_ => return None, }) }
pub(in crate::native_face_scene) fn for_intent(intent: &str) -> Option<Icon> { Some(match intent {
${Object.entries(data.intents).map(([intent,key])=>`${JSON.stringify(intent)} => Icon::${key},`).join('\n')}
_ => return None, }) }
`);
const index = rust(`${header}use super::Icon;
pub(super) const fn index(icon: Icon) -> usize { match icon {
${keys.map((key,i)=>`Icon::${key} => ${i},`).join('\n')}
} }
`);
const browser = `${header}const geometry = ${JSON.stringify(geometry)};
const roles = ${JSON.stringify(data.roles)};
const intents = ${JSON.stringify(data.intents)};
export const iconForRole = role => typeof role === 'string' && Object.hasOwn(roles, role) ? roles[role] : undefined;
export const iconForIntent = intent => typeof intent === 'string' && Object.hasOwn(intents, intent) ? intents[intent] : undefined;
export const iconForKey = key => typeof key === 'string' && Object.hasOwn(geometry, key) ? key : undefined;
export function decorate(node, key) {
  if (!iconForKey(key)) return node;
  const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  for (const [name, value] of Object.entries({ viewBox: '0 0 24 24', width: '18', height: '18', fill: 'none',
    stroke: 'currentColor', 'stroke-width': '1.75', 'stroke-linecap': 'round',
    'stroke-linejoin': 'round', 'aria-hidden': 'true', focusable: 'false', class: 'owner-face-icon' })) {
    icon.setAttribute(name, value);
  }
  // Only repository-owned, checked constants reach this geometry sink.
  icon.innerHTML = geometry[key];
  node.prepend(icon);
  return node;
}
`;
const license = await readFile(new URL('LICENSE', import.meta.url), 'utf8');
const outputs = [
 ['targets/browser/handbook/LUCIDE-LICENSE', license],
 ['targets/conduitos/assets/icons/LICENSE', license],
 ['targets/conduitos/src/native_face_scene/semantic_icons/generated.rs', native],
 ['targets/conduitos/src/display/icons/lucide_index.rs', index],
 ['targets/browser/handbook/owner-face-icons.mjs', browser],
];
for (const [path, bytes] of outputs) {
  const url = new URL(path, root);
  if (process.argv.includes('--check')) {
    if (await readFile(url, 'utf8') !== bytes) throw new Error(`Generated mapping differs: ${path}`);
  } else {
    await mkdir(new URL('./', url), { recursive: true });
    await writeFile(url, bytes);
  }
}
console.log(`${keys.length} icon keys and shared role/intent mappings ${process.argv.includes('--check') ? 'verified' : 'prepared'}`);
