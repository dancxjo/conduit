import net from 'node:net';
import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';
const require = createRequire('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/stage04-refresh/proof/browser/package.json');
const { chromium } = require('@playwright/test');
const delays = [0, 5, 25, 75];
const server = net.createServer(socket => {
  socket.once('data', data => {
    const route = data.toString().split(' ')[1];
    const delay = Number(new URL(route, 'http://localhost').searchParams.get('delay') ?? 0);
    const body = route.startsWith('/asset') ? 'a'.repeat(4096) : '<!doctype html><html><body>Timing diagnostic</body></html>';
    socket.write(`HTTP/1.1 200 OK\r\nContent-Length: ${Buffer.byteLength(body)}\r\n`);
    setTimeout(() => socket.end(`Content-Type: ${route.startsWith('/asset') ? 'text/plain' : 'text/html'}\r\nConnection: close\r\n\r\n${body}`), delay);
  });
  socket.on('error', () => {});
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const browser = await chromium.launch();
const results = [];
try {
  const page = await browser.newPage();
  const root = `http://127.0.0.1:${server.address().port}`;
  await page.goto(root);
  for (const delay of delays) {
    const url = `${root}/asset?delay=${delay}`;
    const terminal = new Promise(resolve => {
      const done = kind => async request => {
        if (request.url() !== url) return;
        page.off('requestfailed', failed); page.off('requestfinished', finished);
        const response = await request.response();
        resolve({ kind, url, errorText: request.failure()?.errorText, ...request.timing(), status: response?.status() });
      };
      const failed = done('requestfailed'); const finished = done('requestfinished');
      page.on('requestfailed', failed); page.on('requestfinished', finished);
    });
    const bytes = await page.evaluate(async url => {
      const response = await fetch(url, { cache: 'no-store' });
      const reader = response.body.getReader(); let bytes = 0;
      try { while (true) { const chunk = await reader.read(); if (chunk.done) break; bytes += chunk.value.byteLength; } }
      finally { reader.releaseLock(); }
      return bytes;
    }, url);
    const request = await terminal;
    const timing = await page.evaluate(url => ({ timeOrigin: performance.timeOrigin, resource: performance.getEntriesByName(url)[0].toJSON() }), url);
    results.push({ delay, bytes, request, ...timing,
      sendDelta: timing.timeOrigin + timing.resource.requestStart - request.startTime - request.requestStart,
      headerDelta: timing.timeOrigin + timing.resource.responseStart - request.startTime - request.responseStart });
  }
  writeFileSync('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/outputs/6055-fragmented-header-diagnostic.json', JSON.stringify({ proofClass: 'local pinned Chromium raw-HTTP timing diagnostic, not product acceptance', results }, null, 2));
  console.log(JSON.stringify(results.map(({ delay, bytes, request, sendDelta, headerDelta }) => ({ delay, bytes, terminal: request.kind, sendDelta, headerDelta })), null, 2));
} finally { await browser.close(); server.close(); }
