// Application document policy. Reviewed SDK modules use blob imports and Wasm;
// neither inline JavaScript nor JavaScript string evaluation is required.
export const STATIC_APPLICATION_CSP = [
  "default-src 'none'", "base-uri 'none'", "object-src 'none'", "frame-src 'none'",
  "form-action 'none'", "connect-src 'self'",
  "script-src 'self' blob: 'wasm-unsafe-eval'",
  "style-src 'self' blob: 'unsafe-inline'",
  "img-src 'self' data: blob:", "font-src 'self' data:", "media-src 'self' blob:",
  "worker-src 'none'",
].join('; ');

export function applyStaticApplicationCsp(html) {
  if (!/<head\b[^>]*>/i.test(html)) throw new Error('Static application HTML requires an explicit head for CSP');
  // Additional authored policies remain in force; never loosen or erase them.
  return html.replace(/<head\b[^>]*>/i, head => `${head}<meta http-equiv="Content-Security-Policy" content="${STATIC_APPLICATION_CSP}">`);
}
