// Private external proof coordinator. No browser/owner is started by --self-test.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstat, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const SOURCE = '253dcb985eb8890ddf873d672d3a282f2366e6db';
const LIMIT = Object.freeze({ line: 4096, commands: 48, pending: 8,
  json: 1024 * 1024, report: 256 * 1024, actionMs: 12000,
  readinessMs: 120000, sessionMs: 1800000 });
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

export function parseCommand(line) {
  assert.ok(Buffer.byteLength(line) <= LIMIT.line, 'command bound');
  const c = JSON.parse(line);
  assert.ok(c && !Array.isArray(c) && typeof c === 'object');
  assert.ok(['handoff', 'snapshot', 'stop'].includes(c.op), 'unknown operation');
  const keys = c.op === 'handoff' ? ['op', 'id', 'standby']
    : c.op === 'snapshot' ? ['op', 'id', 'expected_show_id', 'expected_route_plan_id', 'expected_face_id', 'expected_face_revision', 'after_revision'] : ['op', 'id'];
  assert.ok(Object.keys(c).every(k => keys.includes(k)), 'unexpected field');
  assert.match(c.id, /^[a-z][a-z0-9-]{0,47}$/);
  if (c.op === 'handoff') assert.ok(path.isAbsolute(c.standby));
  if (c.op === 'snapshot') {
    assert.equal(typeof c.expected_show_id, 'string');
    assert.ok(c.expected_show_id.length > 0 && c.expected_show_id.length <= 256);
    for (const key of ['expected_route_plan_id', 'expected_face_id']) {
      assert.equal(typeof c[key], 'string'); assert.ok(c[key].length > 0 && c[key].length <= 256);
    }
    assert.match(c.expected_face_revision, /^\d{1,20}$/);
    assert.match(c.after_revision, /^\d{1,20}$/);
  }
  return c;
}

export function routesForHandoff(report) {
  assert.equal(report.schema, 'conduit.body/owner-mask-wardrobe@1');
  assert.match(report.wardrobe_revision_decimal, /^\d{1,20}$/);
  assert.ok(report.owner_plan_id);
  const descriptions = report.route_descriptions.filter(r => r.mask_name === 'native-graphical');
  assert.equal(descriptions.length, 1, 'exactly one native route required');
  const native = report.admitted_routes.find(r =>
    r.route_id === descriptions[0].route_id && r.currently_available);
  assert.ok(native, 'native route must be sealed and currently available');
  const current = report.route_descriptions.find(r => r.route_id === report.selected?.route_id);
  assert.ok(current && current.route_id !== native.route_id, 'browser must initially be selected');
  return { native, current };
}

export function parseLosslessJSON(text) {
  return JSON.parse(text, (_key, value, context) => {
    if (typeof value === 'number') {
      assert.ok(Number.isFinite(value), 'nonfinite JSON numeric carrier');
      if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
        assert.equal(typeof context?.source, 'string', 'lossless JSON reviver required');
        return context.source;
      }
    }
    return value;
  });
}

export function validateSnapshot(report, command, bodyId, previous, independent) {
  assert.equal(command.after_revision, previous.revision, 'snapshot baseline differs');
  assert.equal(report.schema, 'conduit.body/owner-mask-wardrobe@1');
  assert.equal(report.body_id, bodyId);
  assert.equal(report.face_id, command.expected_face_id);
  assert.equal(independent.schema, 'conduit.body/local-face-snapshot@1');
  assert.equal(independent.face.basis.body_id, bodyId);
  assert.equal(independent.face.identity, command.expected_face_id);
  assert.equal(independent.face_revision_decimal, command.expected_face_revision);
  assert.match(independent.face_revision_decimal, /^(0|[1-9]\d{0,19})$/);
  assert.equal(String(independent.face.revision), independent.face_revision_decimal,
    'independent lossless numeric carrier differs from decimal');
  assert.equal(typeof report.face_revision, 'number', 'DOM report numeric carrier required');
  assert.ok(Number.isFinite(report.face_revision) && Number.isInteger(report.face_revision));
  const browserRevisionExact = Number.isSafeInteger(report.face_revision);
  if (browserRevisionExact) assert.equal(String(report.face_revision), independent.face_revision_decimal);
  // Unsafe DOM numbers are diagnostic only. The independently exported decimal
  // and losslessly parsed CLI Face supply the exact revision binding.
  assert.equal(report.show_id, command.expected_show_id);
  assert.equal(report.selected?.route_id, `route/${command.expected_route_plan_id}`);
  assert.ok(report.owner_plan_id);
  assert.match(report.wardrobe_revision_decimal, /^\d{1,20}$/);
  // A completed explicit Inspect can truthfully reread unchanged state after
  // a provisional-input refusal. Native QMP proof establishes actual changes.
  return { owner_plan_id: report.owner_plan_id, revision: report.wardrobe_revision_decimal,
    show_id: report.show_id, revision_binding: {
      source: 'installed-owner-cli-local-face-snapshot', face_id: independent.face.identity,
      face_revision_decimal: independent.face_revision_decimal,
      browser_numeric_revision_exact: browserRevisionExact } };
}

async function privateDirectory(dir) {
  const s = await lstat(dir);
  assert.ok(s.isDirectory() && !s.isSymbolicLink());
  assert.equal(s.mode & 0o077, 0, 'private directory required');
  assert.equal(s.uid, process.getuid(), 'directory must belong to current user');
}
async function boundedJSON(file, max = LIMIT.json) {
  const s = await lstat(file);
  assert.ok(s.isFile() && !s.isSymbolicLink() && s.size <= max);
  return parseLosslessJSON(await readFile(file, 'utf8'));
}
function runJSON(executable, args) {
  const result = spawnSync(executable, args, {
    encoding: 'utf8', timeout: 10000, maxBuffer: LIMIT.json,
  });
  assert.equal(result.status, 0, 'installed owner observation failed');
  return parseLosslessJSON(result.stdout);
}

export async function coordinate(page, before, awaitRevision) {
  const { native, current } = routesForHandoff(before);
  // Each occurrence is sent once. A timeout is terminal; never replay it.
  await page.getByRole('button', { name: 'Wear native-graphical', exact: true }).click();
  const worn = await awaitRevision(before.wardrobe_revision_decimal);
  assert.equal(worn.owner_plan_id, before.owner_plan_id);
  assert.equal(worn.selected?.route_id, before.selected.route_id);
  await page.getByRole('button', { name: `Doff ${current.mask_name}`, exact: true }).click();
  const doffed = await awaitRevision(worn.wardrobe_revision_decimal);
  assert.equal(doffed.owner_plan_id, before.owner_plan_id);
  assert.equal(doffed.selected?.route_id, native.route_id);
  await page.getByRole('button', { name: 'Prefer only native-graphical', exact: true }).click();
  const preferred = await awaitRevision(doffed.wardrobe_revision_decimal);
  assert.equal(preferred.owner_plan_id, before.owner_plan_id);
  assert.equal(preferred.selected?.route_id, native.route_id);
  return { before, worn, doffed, preferred };
}

async function selfTest() {
  assert.equal(parseCommand('{"op":"stop","id":"done"}').op, 'stop');
  for (const invalid of [
    '{"op":"wear","id":"a"}', '{"op":"stop","id":"../secret"}',
    '{"op":"handoff","id":"a","standby":"relative"}',
    '{"op":"snapshot","id":"a","after_revision":"-1","expected_show_id":"x"}',
    '{"op":"stop","id":"a","token":"secret"}', ' '.repeat(LIMIT.line + 1),
  ]) assert.throws(() => parseCommand(invalid));
  const report = { schema: 'conduit.body/owner-mask-wardrobe@1', owner_plan_id: 'body-plan',
    wardrobe_revision_decimal: '1', selected: { route_id: 'browser' },
    route_descriptions: [{ route_id: 'browser', mask_name: 'browser-dom' },
      { route_id: 'native', mask_name: 'native-graphical' }],
    admitted_routes: [{ route_id: 'native', currently_available: true }] };
  const calls = [];
  const page = { getByRole: (role, options) => ({ click: async () => calls.push(options.name) }) };
  let n = 1;
  await coordinate(page, report, async prior => {
    assert.equal(prior, String(n)); n++;
    return { ...report, wardrobe_revision_decimal: String(n),
      selected: { route_id: n === 2 ? 'browser' : 'native' } };
  });
  assert.deepEqual(calls, ['Wear native-graphical', 'Doff browser-dom', 'Prefer only native-graphical']);
  const failCalls = [];
  await assert.rejects(coordinate({ getByRole: (_, o) => ({ click: async () => failCalls.push(o.name) }) },
    report, async () => { throw new Error('timeout'); }));
  assert.deepEqual(failCalls, ['Wear native-graphical'], 'no retry after timeout');
  assert.throws(() => routesForHandoff({ ...report, admitted_routes: [] }));
  const command = parseCommand(JSON.stringify({ op: 'snapshot', id: 'look', expected_show_id: 'show',
    expected_route_plan_id: 'child', expected_face_id: 'face', expected_face_revision: '7', after_revision: '9' }));
  const snapshot = { ...report, owner_plan_id: 'new-outer-plan', wardrobe_revision_decimal: '0',
    body_id: 'body', face_id: 'face', face_revision: 7, show_id: 'show', selected: { route_id: 'route/child' } };
  const previous = { owner_plan_id: 'old-outer-plan', revision: '9', show_id: 'old-show' };
  const independent = { schema: 'conduit.body/local-face-snapshot@1', face_revision_decimal: '7',
    face: { identity: 'face', revision: 7, basis: { body_id: 'body' } } };
  assert.equal(validateSnapshot(snapshot, command, 'body', previous, independent).revision, '0');
  for (const change of [{ body_id: 'other' }, { face_id: 'other' }, { face_revision: 8 },
    { show_id: 'old-show' }, { selected: { route_id: 'route/other' } }]) {
    assert.throws(() => validateSnapshot({ ...snapshot, ...change }, command, 'body', previous, independent));
  }
  assert.equal(validateSnapshot({ ...snapshot, owner_plan_id: 'old-outer-plan',
    wardrobe_revision_decimal: '9' }, command, 'body', { ...previous, show_id: 'show' }, independent).revision, '9',
    'fresh explicit inspection of unchanged state is valid');
  assert.throws(() => validateSnapshot({ ...snapshot, show_id: 'old-show' }, command, 'body', previous, independent),
    'old cached Show cannot satisfy a requested new acknowledged Show');
  const decimal = '6577474094915678572';
  assert.equal(parseLosslessJSON(`{\"x\":${decimal}}`).x, decimal);
  assert.equal(parseLosslessJSON('{\"maximum_u64\":18446744073709551615}').maximum_u64, '18446744073709551615');
  assert.equal(parseLosslessJSON('{\"safe\":7}').safe, 7);
  const large = parseLosslessJSON(JSON.stringify(independent).replace('\"revision\":7', `\"revision\":${decimal}`));
  large.face_revision_decimal = decimal;
  const bigCommand = { ...command, expected_face_revision: decimal };
  const rounded = { ...snapshot, face_revision: Number(decimal) };
  const big = validateSnapshot(rounded, bigCommand, 'body', previous, large);
  assert.equal(big.revision_binding.face_revision_decimal, decimal);
  assert.equal(big.revision_binding.browser_numeric_revision_exact, false);
  for (const wrong of [{ ...large, face_revision_decimal: '6577474094915678573' },
    { ...large, face: { ...large.face, identity: 'wrong-face' } },
    { ...large, face: { ...large.face, revision: '6577474094915678573' } }]) {
    assert.throws(() => validateSnapshot(rounded, bigCommand, 'body', previous, wrong));
  }
  process.stdout.write('synthetic command/sequence/bounds checks passed; no browser or owner launched\n');
}

async function main(args) {
  assert.equal(args.length, 6,
    'usage: node HELPER REPO INSTALLED-OWNER PRIVATE-STATE HANDBOOK PINNED-PLAYWRIGHT NEW-PRIVATE-OUTPUT');
  const [repo, owner, state, handbook, playwright, output] = args.map(p => path.resolve(p));
  await privateDirectory(state);
  await privateDirectory(path.dirname(output));
  const head = spawnSync('git', ['-C', repo, 'rev-parse', 'HEAD'], { encoding: 'utf8', timeout: 10000, maxBuffer: LIMIT.json });
  assert.equal(head.status, 0); assert.equal(head.stdout.trim(), SOURCE, 'frozen Source differs');
  const dirty = spawnSync('git', ['-C', repo, 'status', '--porcelain'], { encoding: 'utf8', timeout: 10000, maxBuffer: LIMIT.json });
  assert.equal(dirty.status, 0); assert.equal(dirty.stdout, '', 'Source must be clean');
  const installed = await boundedJSON(path.join(state, 'installation.json'));
  assert.equal(installed.product_executable, owner);
  assert.equal(installed.release_source_identity, SOURCE, 'installed sealed Source differs');
  const expected = (await boundedJSON(path.join(repo, 'proof/browser/package.json')))
    .devDependencies['@playwright/test'];
  assert.equal((await boundedJSON(path.join(path.dirname(playwright), 'package.json'))).version, expected);
  await mkdir(output, { mode: 0o700 }); // exclusive: do not overwrite a previous proof
  const { startStaticProduct } = await import(pathToFileURL(path.join(repo, 'proof/browser/static-product-server.mjs')));
  const { chromium } = await import(pathToFileURL(playwright));
  const ownerRead = command => runJSON(owner, ['body', command, '--state-dir', state, '--json']);
  let server, browser, timer, stopId;
  const abort = new AbortController();
  const terminate = () => abort.abort(new Error('controller terminated'));
  process.once('SIGINT', terminate); process.once('SIGTERM', terminate);
  const respond = value => process.stdout.write(`${JSON.stringify(value)}\n`);
  try {
    timer = setTimeout(() => abort.abort(new Error('session deadline')), LIMIT.sessionMs);
    server = await startStaticProduct(handbook);
    // Same pinned Chromium launch as the existing journey, one context/page,
    // no runner concurrency or retries. No force or action-performing polling.
    browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
    const page = await context.newPage();
    page.setDefaultTimeout(LIMIT.actionMs);
    const errors = [];
    page.on('pageerror', () => { if (errors.length < 16) errors.push('pageerror'); });
    const initial = ownerRead('status');
    const bodyId = initial.biography.body_id;
    // Actual installed status shape: controller::truth -> realization -> BodyPlan.
    const workloadPlan = initial.realization?.plan?.plan_id ?? null;
    const checkWorkload = () => {
      const current = ownerRead('status');
      assert.equal(current.biography.body_id, bodyId);
      assert.equal(current.realization?.plan?.plan_id ?? null, workloadPlan, 'workload BodyPlan changed');
      return { available: workloadPlan !== null, plan_id: workloadPlan };
    };
    assert.equal(initial.biography.membership.parts.filter(p => p.current).length, 1,
      'start browser before guest, with only owner present');
    await page.goto(`${server.url}?participate=owner#your-handbook`);
    await page.locator('[data-owner-key]').waitFor();
    const identity = await page.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
    const window = runJSON(owner, ['body', 'browser-window', '--state-dir', state,
      '--expected-host-id', identity.hostId, '--new-host-verifying-key', JSON.stringify(identity.verifyingKey),
      '--maximum-millis', '60000', '--authorize-window']);
    assert.equal(window.body_id, bodyId);
    await page.getByLabel('Body ID').fill(bodyId);
    await page.getByLabel('Owner window URL').fill(window.url);
    await page.getByRole('button', { name: 'Join this Body', exact: true }).click();
    await page.waitForFunction(() => globalThis.__conduitOwnerParticipation?.presence() === 'available',
      null, { timeout: LIMIT.actionMs });
    await page.locator('[data-owner-face-document] [data-owner-action]').first().waitFor();
    const joined = await page.evaluate(() => ({
      partId: globalThis.__conduitOwnerParticipation.credential().part_id,
      face: globalThis.__conduitOwnerParticipation.face(),
    }));
    assert.equal(joined.face.body_id, bodyId);
    assert.equal(joined.face.show_state, 'available');
    assert.equal(ownerRead('status').biography.membership.parts.filter(p => p.current).length, 2,
      'guest must not start until browser-joined response');
    const save = async (id, value) => {
      assert.equal(errors.length, 0, 'browser page error');
      const bytes = Buffer.from(`${JSON.stringify(value, null, 2)}\n`);
      assert.ok(bytes.length <= LIMIT.report, 'report bound');
      const filename = `${id}.json`;
      await writeFile(path.join(output, filename), bytes, { mode: 0o600, flag: 'wx' });
      return { file: filename, bytes: bytes.length, sha256: hash(bytes) };
    };
    const joinedArtifact = await save('browser-joined', { source: SOURCE,
      release_source_identity: installed.release_source_identity, body_id: bodyId,
      browser_host_id: identity.hostId, browser_part_id: joined.partId, face: joined.face,
      proof: 'private-browser-session', physical: false, human: false });
    respond({ event: 'browser-joined', artifact: joinedArtifact });
    const readWardrobe = async () => JSON.parse(await page.locator('[data-owner-wardrobe-evidence]').textContent());
    const awaitRevision = async prior => {
      await page.waitForFunction(revision => {
        try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
          .wardrobe_revision_decimal !== revision; } catch { return false; }
      }, prior, { timeout: LIMIT.actionMs });
      return readWardrobe();
    };
    const inspectOnce = async () => {
      await page.getByRole('button', { name: 'Inspect current wardrobe', exact: true }).click();
      // Click listener synchronously disables the button until the request ends.
      await page.getByRole('button', { name: 'Inspect current wardrobe', exact: true }).isEnabled()
        .then(async enabled => { if (!enabled) await page.waitForFunction(() =>
          !document.querySelector('[data-owner-wardrobe-refresh]').disabled,
        null, { timeout: LIMIT.actionMs }); });
      const report = await readWardrobe();
      assert.equal(report.schema, 'conduit.body/owner-mask-wardrobe@1');
      assert.notEqual(await page.locator('[data-owner-wardrobe-status]').getAttribute('data-refused'), 'true',
        'inspection refused; previous DOM report is not current proof');
      return report;
    };
    let handedOff = false, previous;
    const used = new Set(['browser-joined']);
    let buffer = '', pending = [], commandCount = 0, wake;
    let ended = false, inputError;
    const notify = () => { wake?.(); wake = undefined; };
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', chunk => {
      buffer += chunk;
      try {
        assert.ok(Buffer.byteLength(buffer) <= LIMIT.line * LIMIT.pending, 'input buffer bound');
        let end;
        while ((end = buffer.indexOf('\n')) >= 0) {
          const line = buffer.slice(0, end); buffer = buffer.slice(end + 1);
          assert.ok(++commandCount <= LIMIT.commands && pending.length < LIMIT.pending, 'command queue bound');
          pending.push(parseCommand(line));
        }
        assert.ok(Buffer.byteLength(buffer) <= LIMIT.line, 'partial command bound');
      } catch { inputError = new Error('invalid or excessive control input'); }
      notify();
    });
    process.stdin.on('end', () => { ended = true; notify(); });
    process.stdin.on('error', () => { inputError = new Error('control input failed'); notify(); });
    abort.signal.addEventListener('abort', notify);
    for (;;) {
      if (inputError) throw inputError;
      if (abort.signal.aborted) throw new Error('session deadline');
      if (!pending.length) {
        if (ended) throw new Error('control EOF without explicit stop');
        await new Promise(resolve => { wake = resolve; }); continue;
      }
      const c = pending.shift();
      assert.ok(!used.has(c.id), 'duplicate command id'); used.add(c.id);
      if (c.op === 'stop') { stopId = c.id; break; }
      if (c.op === 'handoff') {
        assert.equal(handedOff, false, 'handoff occurs once');
        await privateDirectory(path.dirname(c.standby));
        const privateFile = await lstat(c.standby);
        assert.equal(privateFile.uid, process.getuid());
        assert.equal(privateFile.mode & 0o077, 0, 'standby must be private');
        const standby = await boundedJSON(c.standby);
        assert.equal(standby.stage, 'standby');
        assert.equal(standby.guest_part.membership_installed, true);
        assert.equal(standby.guest_part.body_id, bodyId);
        assert.equal(standby.face.interactions_admitted, false);
        assert.notEqual(standby.guest_part.part_id, joined.partId);
        const deadline = Date.now() + LIMIT.readinessMs;
        let ready = false;
        while (Date.now() < deadline && !abort.signal.aborted) {
          // Read-only installed owner membership observation; no UI action here.
          const status = ownerRead('status');
          ready = status.biography.membership.parts.some(p =>
            p.part_id === standby.guest_part.part_id && p.current);
          if (ready) break;
          await sleep(250);
        }
        assert.ok(ready, 'native passive membership readiness deadline');
        const before = await inspectOnce();
        const result = await coordinate(page, before, awaitRevision);
        previous = { owner_plan_id: result.preferred.owner_plan_id,
          revision: result.preferred.wardrobe_revision_decimal, show_id: result.preferred.show_id };
        const workload = checkWorkload();
        handedOff = true;
        respond({ id: c.id, event: 'native-preferred', wardrobe_revision: previous.revision, owner_plan_id: previous.owner_plan_id,
          artifact: await save(c.id, { source: SOURCE, body_id: bodyId, workload, ...result }) });
      } else {
        assert.ok(handedOff, 'snapshot requires completed handoff');
        assert.equal(c.after_revision, previous.revision, 'snapshot must name last observed revision');
        const report = await inspectOnce(); // one deliberate observation, not a wait action
        // Exactly one independent read per requested snapshot, never in a poll.
        const independent = ownerRead('face');
        previous = validateSnapshot(report, c, bodyId, previous, independent);
        const workload = checkWorkload();
        respond({ id: c.id, event: 'snapshot', wardrobe_revision: previous.revision, owner_plan_id: previous.owner_plan_id,
          artifact: await save(c.id, { source: SOURCE, body_id: bodyId, workload, report,
            revision_binding: previous.revision_binding }) });
      }
    }
  } finally {
    clearTimeout(timer);
    try { await browser?.close(); } finally {
      if (server?.child && server.child.exitCode === null) {
        const stopped = new Promise(resolve => server.child.once('exit', resolve));
        server.child.kill();
        await Promise.race([stopped, sleep(5000).then(() => server.child.kill('SIGKILL'))]);
      }
      process.stdin.pause();
      process.removeListener('SIGINT', terminate); process.removeListener('SIGTERM', terminate);
    }
    if (stopId) respond({ id: stopId, event: 'stopped' });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    if (process.argv.slice(2).join(' ') === '--self-test') await selfTest();
    else await main(process.argv.slice(2));
  } catch {
    // Deliberately omit raw browser/owner errors: they can include private URLs.
    process.stderr.write('owner-browser-session failed; private run stopped; no occurrence replayed\n');
    process.exitCode = 1;
  }
}
