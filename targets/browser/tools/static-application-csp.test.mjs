import test from 'node:test';
import assert from 'node:assert/strict';
import { applyStaticApplicationCsp, STATIC_APPLICATION_CSP } from './static-application-csp.mjs';

test('policy precedes executable assets while preserving SVG and authored markup', () => {
  const source = '<!doctype html><html><HEAD><script type="module" src="app.mjs"></script><style>svg{width:100%}</style></HEAD><body><svg><path d="M0 0"/></svg></body></html>';
  const secured = applyStaticApplicationCsp(source);
  assert(secured.indexOf('Content-Security-Policy') < secured.indexOf('<script'));
  assert.equal(secured.replace(/<meta http-equiv="Content-Security-Policy"[^>]*>/, ''), source);
});

test('policy permits reviewed module and Wasm loading without script eval or backend origins', () => {
  const directives = new Map(STATIC_APPLICATION_CSP.split('; ').map(value => {
    const [name, ...sources] = value.split(' '); return [name, sources];
  }));
  assert.deepEqual(directives.get('script-src'), ["'self'", 'blob:', "'wasm-unsafe-eval'"]);
  assert.deepEqual(directives.get('connect-src'), ["'self'"]);
  for (const name of ['default-src', 'object-src', 'frame-src', 'worker-src', 'form-action', 'base-uri']) {
    assert.deepEqual(directives.get(name), ["'none'"]);
  }
  assert(!STATIC_APPLICATION_CSP.includes("'unsafe-eval'"));
  assert(directives.get('style-src').includes("'unsafe-inline'"));
});

test('existing tighter policy survives and headless fragments refuse packaging', () => {
  const existing = '<meta http-equiv="Content-Security-Policy" content="default-src &#39;none&#39;">';
  assert(applyStaticApplicationCsp(`<html><head>${existing}</head></html>`).includes(existing));
  assert.throws(() => applyStaticApplicationCsp('<svg/>'), /explicit head/);
});

test('explicit owner window widens only its document to bounded loopback WebSockets', () => {
  const source = '<html><head></head><body></body></html>';
  const secured = applyStaticApplicationCsp(source, { loopbackOwnerWindow: true });
  assert.match(secured, /connect-src 'self' ws:\/\/127\.0\.0\.1:\*/);
  assert.doesNotMatch(applyStaticApplicationCsp(source), /ws:\/\/127\.0\.0\.1/);
  assert.doesNotMatch(secured, /connect-src[^;]*wss:\/\//);
});
