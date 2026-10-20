import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import net from 'node:net';
import { randomUUID } from 'node:crypto';
import { startStaticProduct } from './native-panels/proof/browser/static-product-server.mjs';
const require = createRequire('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/stage04-refresh/proof/browser/package.json');
const { chromium } = require('@playwright/test');
assert.equal(require('@playwright/test/package.json').version, '1.62.0');
const directory = await mkdtemp('/tmp/conduit-transfer-identity-');
await writeFile(`${directory}/index.html`, '<!doctype html><title>Response identity diagnostic</title>');
await writeFile(`${directory}/data.txt`, 'a'.repeat(4096));
const entrance = await startStaticProduct(directory);
const incarnation = randomUUID();
let sequence = 0;
const fragmented = net.createServer(socket => {
  socket.once('data', () => {
    const identity = `${incarnation}:${++sequence}`;
    socket.write(`HTTP/1.1 200 OK\r\nContent-Length: 4096\r\n`);
    setTimeout(() => socket.end(`Content-Type: text/plain\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Expose-Headers: X-Conduit-Proof-Transfer\r\nTiming-Allow-Origin: *\r\nX-Conduit-Proof-Transfer: ${identity}\r\nServer-Timing: conduit-transfer;desc="${identity}"\r\nConnection: close\r\n\r\n${'a'.repeat(4096)}`), 25);
  });
  socket.on('error', () => {});
});
await new Promise(resolve => fragmented.listen(0, '127.0.0.1', resolve));
let browser;
const observations = [];
try {
  browser = await chromium.launch();
  const page = await browser.newPage();
  await page.goto(entrance.url);
  for (const url of [new URL('data.txt', entrance.url).href,
    `http://127.0.0.1:${fragmented.address().port}/data.txt`]) {
    const identities = [];
    for (let occurrence = 0; occurrence < 2; occurrence++) {
      const observation = await page.evaluate(async url => {
        const response = await fetch(url, { cache: 'no-store' });
        const identity = response.headers.get('x-conduit-proof-transfer');
        const bytes = (await response.arrayBuffer()).byteLength;
        const resources = performance.getEntriesByName(url);
        return { url, identity, bytes, resource: resources.at(-1).toJSON() };
      }, url);
      assert.equal(observation.bytes, 4096);
      assert.equal(observation.resource.responseStatus, 200);
      assert.equal(observation.resource.encodedBodySize, 4096);
      assert.equal(observation.resource.decodedBodySize, 4096);
      const metrics = observation.resource.serverTiming.filter(entry => entry.name === 'conduit-transfer');
      assert.equal(metrics.length, 1);
      assert.equal(metrics[0].description, observation.identity);
      assert.ok(observation.identity);
      identities.push(observation.identity);
      observations.push({ occurrence, ...observation });
    }
    assert.notEqual(identities[0], identities[1]);
  }
  await writeFile('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/outputs/6055-transfer-identity-chromium.json', JSON.stringify({
    proofClass: 'local pinned Chromium proof-server/fragmented-header identity diagnostic, not product acceptance or abort enactment',
    playwright: require('@playwright/test/package.json').version,
    browser: browser.version(), observations,
  }, null, 2));
  console.log(JSON.stringify({ status: 'passed', observations: observations.length, browser: browser.version() }));
} finally {
  await browser?.close();
  entrance.child.kill();
  fragmented.close();
  await rm(directory, { recursive: true });
}
