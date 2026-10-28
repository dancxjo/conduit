import { nativeRecords } from './native-records.mjs';
import { parseCaptureJSON, captureDecimal } from '../browser/exact-capture-json.mjs';
// Live QMP notebook acceptance through the installed owner. All domain state stays in the installed owner.
import assert from 'node:assert/strict';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { openSync, closeSync, existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { setTimeout as pause } from 'node:timers/promises';

export async function prepareNative({ installed, state, bodyId, build, privateRoot, output, stop, xtask }) {
  const directory = path.join(privateRoot, 'native'); await mkdir(directory, { mode: 0o700 });
  const evidence = path.join(output, 'native'); await mkdir(evidence);
  const children = [], qmpLog = [], policyEvidence = [];
  let socket, guestProcess;
  const call = (exe, args) => {
    const run = spawnSync(exe, args, { encoding: 'utf8', timeout: 120000, maxBuffer: 2 * 1024 * 1024 });
    assert.equal(run.status, 0, `${args.join(' ')}: ${run.stderr}`);
    return run.stdout;
  };
  const wait = async (predicate, label, timeout = 120000) => {
    const until = Date.now() + timeout;
    while (Date.now() < until) {
      for (const child of children) {
        if (child !== guestProcess && child.exitCode === 0) continue;
        assert.ok(child.exitCode === null && child.signalCode === null, `${label}: ${child.spawnargs[1]} exited (${child.exitCode}, ${child.signalCode})`);
      }
      const result = await predicate(); if (result) return result;
      await pause(50);
    }
    throw new Error(`native deadline: ${label}`);
  };
  const records = async prefix => {
    if (!existsSync(serial)) return [];
    const text = await readFile(serial, 'utf8');
    return nativeRecords(text, prefix);
  };
  const serial = path.join(evidence, 'guest.serial.log');
  try {
    const hostIp = Object.values(os.networkInterfaces()).flat().find(address => address.family === 'IPv4' && !address.internal)?.address;
    assert.ok(hostIp && /^(10\.|172\.(1[6-9]|2\d|3[01])\.|192\.168\.)/.test(hostIp), 'explicit private owner IPv4');
    const port = await new Promise((resolve, reject) => {
      const probe = net.createServer(); probe.once('error', reject);
      probe.listen(0, hostIp, () => { const port = probe.address().port; probe.close(() => resolve(port)); });
    });
    const cert = path.join(directory, 'route.pem'), key = path.join(directory, 'route.key');
    call('openssl', ['req', '-x509', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:P-256', '-nodes',
      '-keyout', key, '-out', cert, '-days', '1', '-subj', '/CN=10.0.2.42',
      '-addext', `subjectAltName=IP:10.0.2.42,IP:${hostIp}`]);
    const invitation = path.join(directory, 'invitation.json');
    const inviteFd = openSync(invitation, 'wx', 0o600), inviteErr = openSync(path.join(directory, 'invite.stderr'), 'wx', 0o600);
    const listener = spawn(installed, ['body', 'invite', '--state-dir', state, '--ttl-seconds', '600',
      '--route-bind', `${hostIp}:${port}`, '--route-url', 'wss://10.0.2.42:4433/conduit',
      '--route-tls-cert', cert, '--route-tls-key', key, '--authorize-route'], { stdio: ['ignore', inviteFd, inviteErr] });
    closeSync(inviteFd); closeSync(inviteErr); children.push(listener);
    const invite = await wait(async () => {
      try { return JSON.parse(await readFile(invitation, 'utf8')); } catch { return null; }
    }, 'authorized invitation', 10000);
    assert.equal(invite.claim.body_id, bodyId);
    const bodyFile = path.join(directory, 'notebook.body.conduit');
    const profile = path.resolve('targets/conduitos/profiles/conduitos-native.host.conduit');
    await writeFile(bodyFile, `body shared {\n schema = 2\n id = ${JSON.stringify(bodyId)}\n host = {name: "native", configuration: ${JSON.stringify(profile)}, spore: {join_mode: "self-joining", invitation: ${JSON.stringify(invite.claim.invitation_id)}, output: "disk-image"}}\n}\n`, { mode: 0o600 });
    const iso = path.join(directory, 'native.private.iso');
    const provision = parseCaptureJSON(call(xtask, ['--json', 'make', 'body', 'provision-conduitos', bodyFile,
      '--host', 'native', '--build', build, '--invitation', invitation, '--route-tls-cert', cert, '--output', iso]));
    assert.equal(provision.body_id, bodyId); assert.equal(provision.created, true);
    await writeFile(path.join(evidence, 'provision.json'), JSON.stringify(provision, null, 2));
    const buildManifest = JSON.parse(await readFile(path.join(build, 'build-manifest.json'), 'utf8'));
    const qmpPath = path.join(directory, 'qmp.sock');
    const args = ['-M', 'q35', '-cpu', 'max', '-m', '512M', '-smp', '1', '-display', 'none', '-vga', 'std',
      '-monitor', 'none', '-serial', `file:${serial}`, '-no-reboot',
      '-netdev', `user,id=conduit-owner,restrict=on,guestfwd=tcp:10.0.2.42:4433-cmd:nc ${hostIp} ${port}`,
      '-device', 'virtio-net-pci,netdev=conduit-owner,disable-modern=on,rx_queue_size=256,tx_queue_size=256',
      '-qmp', `unix:${qmpPath},server=on,wait=off`, '-device', 'qemu-xhci,id=conduitos-xhci,p2=2,p3=0',
      '-device', 'usb-kbd,bus=conduitos-xhci.0,port=1', '-device', 'usb-mouse,bus=conduitos-xhci.0,port=2',
      '-cdrom', iso, '-boot', 'd'];
    const err = openSync(path.join(evidence, 'qemu.stderr'), 'wx', 0o600);
    const guest = spawn('qemu-system-x86_64', args, { stdio: ['ignore', 'ignore', err] }); guestProcess = guest; closeSync(err); children.push(guest);
    await wait(() => existsSync(qmpPath), 'QMP socket');
    socket = net.createConnection(qmpPath);
    let buffer = '', nextId = 0, pending, connectionError;
    socket.on('error', error => {
      connectionError = error;
      if (pending) { clearTimeout(pending.timer); pending.reject(error); pending = undefined; }
    });
    socket.on('data', chunk => {
      buffer += chunk.toString(); assert.ok(buffer.length <= 1024 * 1024, 'finite QMP frame');
      while (buffer.includes('\n')) {
        const boundary = buffer.indexOf('\n'), line = buffer.slice(0, boundary); buffer = buffer.slice(boundary + 1);
        if (!line.trim()) continue;
        const response = JSON.parse(line); qmpLog.push({ received: response });
        assert.ok(qmpLog.length <= 4096, 'finite QMP observations');
        if (pending && response.id === pending.id) {
          const operation = pending; pending = undefined; clearTimeout(operation.timer);
          response.error ? operation.reject(new Error(JSON.stringify(response.error))) : operation.resolve(response.return);
        }
      }
    });
    const command = (execute, arguments_) => new Promise((resolve, reject) => {
      if (connectionError) { reject(connectionError); return; }
      assert.equal(pending, undefined); const id = ++nextId;
      const timer = setTimeout(() => { pending = undefined; reject(new Error(`QMP ${execute} deadline`)); }, 5000);
      pending = { id, resolve, reject, timer };
      const message = { execute, arguments: arguments_, id }; qmpLog.push({ sent: message });
      socket.write(`${JSON.stringify(message)}\r\n`);
    });
    await command('qmp_capabilities', {});
    const sendKey = (key, down) => command('input-send-event', { events: [{ type: 'key', data: { down, key: { type: 'qcode', data: key } } }] });
    const keyPair = async name => { await sendKey(name, true); await sendKey(name, false); };
    const observedKey = async (name, showId) => {
      const prior = (await records('CONDUIT_CRECHE_FOCUS')).length;
      await sendKey(name, true);
      try {
        const focus = await wait(async () => { const focused = await records('CONDUIT_CRECHE_FOCUS'); return focused.length > prior && focused.at(-1); }, `guest consumed ${name}`, 10000);
        assert.equal(focus.show_id, showId); return focus;
      } finally { await sendKey(name, false); }
    };
    const part = await wait(async () => (await records('CONDUIT_NATIVE_GUEST_PART')).find(part => part.membership_installed), 'guest membership');
    assert.equal(part.body_id, bodyId);
    const standby = await wait(async () => (await records('CONDUIT_NATIVE_OWNER_FACE')).find(face => face.status === 'shown' && face.local_show_available), 'standby Face');
    await writeFile(path.join(evidence, 'admission.json'), JSON.stringify({ part, standby }, null, 2));
    const shot = async name => command('screendump', { filename: path.join(evidence, `${name}.ppm`) });
    const currentFace = () => parseCaptureJSON(call(installed, ['body', 'face', '--state-dir', state, '--json'])).face;
    const notebook = () => JSON.parse(call(installed, ['body', 'notebook', '--state-dir', state, 'show']));
    const awaitShown = async prior => wait(async () => {
      const faces = await records('CONDUIT_NATIVE_OWNER_FACE');
      const shown = faces.findLast(face => face.status === 'shown' && face.interactions_admitted && face.show_id !== prior);
      const ack = shown && faces.findLast(face => face.status === 'acknowledged' && face.show_id === shown.show_id);
      return ack && { shown, ack };
    }, 'acknowledged current native Show', 30000);
    let browserName;
    const refreshBrowser = async page => {
      const expected = currentFace().identity;
      await page.getByRole('button', { name: 'Refresh this Face', exact: true }).click();
      await page.waitForFunction(() => !document.querySelector('[data-owner-face-refresh]').disabled);
      policyEvidence.push({ phase: 'browser-route-preparation', expected_face: expected,
        observation: await page.evaluate(() => ({ face: globalThis.__conduitOwnerParticipation.face(), status: document.querySelector('[data-owner-face-status]')?.textContent })) });
    };
    const wardrobe = page => page.locator('[data-owner-wardrobe-evidence]').textContent().then(parseCaptureJSON);
    const inspect = async page => {
      await refreshBrowser(page);
      await page.getByRole('button', { name: 'Inspect current wardrobe', exact: true }).click();
      await page.waitForFunction(() => !document.querySelector('[data-owner-wardrobe-refresh]').disabled
        && !document.querySelector('[data-owner-wardrobe-status]').textContent.startsWith('Inspecting'));
      const refused = await page.locator('[data-owner-wardrobe-status]').getAttribute('data-refused');
      if (refused) throw new Error(await page.locator('[data-owner-wardrobe-status]').textContent());
      const report = await wardrobe(page); policyEvidence.push(report); return report;
    };
    const choose = async (page, label) => {
      const prior = (await wardrobe(page)).wardrobe_revision_decimal;
      await page.getByRole('button', { name: label, exact: true }).click();
      await page.waitForFunction(prior => JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent).wardrobe_revision_decimal !== prior, prior);
      policyEvidence.push(await wardrobe(page));
    };
    return {
      async wear(page) {
        const initial = await inspect(page);
        assert.ok(initial.route_descriptions.some(route => route.mask_name === 'native-graphical'));
        browserName = initial.route_descriptions.find(route => route.route_id === initial.selected.route_id).mask_name;
        await choose(page, 'Wear native-graphical');
      },
      async edit(page, replacement, staleView) {
        call(installed, ['body', 'lull', '--state-dir', state]);
        await wait(() => !currentFace().subjects.some(subject => subject.identity === 'notebook/document'), 'idle before native wardrobe', 10000);
        await inspect(page);
        await choose(page, `Doff ${browserName}`);
        call(installed, ['body', 'start', '--state-dir', state]);
        await wait(() => currentFace().subjects.some(subject => subject.identity === 'notebook/document'), 'native editing encounter', 10000);
        await keyPair('f5');
        const before = await awaitShown(); const ownerBefore = currentFace();
        assert.equal(before.shown.face_id, ownerBefore.identity); assert.equal(captureDecimal(before.shown.face_revision), captureDecimal(ownerBefore.revision));
        await shot('notebook-native-before');
        const current = notebook();
        assert.equal(current.text, 'I was here. Again.'); assert.equal(current.revision, 2);
        let focus;
        for (let index = 0; index < 5; index++) focus = await observedKey('tab', before.shown.show_id);
        assert.equal(focus.action_id, 'notebook.replace'); assert.equal(focus.argument_name, 'text');
        for (const char of replacement) {
          focus = await observedKey(char === ' ' ? 'spc' : char, before.shown.show_id);
          assert.equal(focus.action_id, 'notebook.replace'); assert.equal(focus.argument_name, 'text');
        }
        await shot('notebook-native-draft');
        assert.equal(notebook().revision, 2);
        await sendKey('ret', true);
        const action = await wait(async () => (await records('CONDUIT_NATIVE_OWNER_ACTION')).at(-1), 'native typed edit', 30000);
        await sendKey('ret', false);
        assert.equal(action.status, 'accepted'); assert.equal(action.action_id, 'notebook.replace');
        assert.equal(action.prior_show_id, before.shown.show_id); assert.equal(action.face_id, before.shown.face_id);
        const after = await awaitShown(before.shown.show_id), ownerAfter = currentFace();
        assert.equal(after.shown.face_id, ownerAfter.identity); assert.notEqual(after.shown.face_id, before.shown.face_id);
        assert.equal(notebook().text, replacement); assert.equal(notebook().revision, 3);
        await shot('notebook-native-after');
        call(installed, ['body', 'lull', '--state-dir', state]);
        await wait(() => !currentFace().subjects.some(subject => subject.identity === 'notebook/document'), 'idle before browser wardrobe', 10000);
        await inspect(page);
        await choose(page, `Wear ${browserName}`); await choose(page, `Prefer only ${browserName}`);
        call(installed, ['body', 'start', '--state-dir', state]);
        await wait(() => currentFace().subjects.some(subject => subject.identity === 'notebook/document'), 'browser editing encounter', 10000);
        await page.getByRole('button', { name: 'Refresh this Face', exact: true }).click();
        await page.waitForFunction(text => globalThis.__conduitOwnerParticipation.face()?.subjects?.some(subject => subject.identity === 'notebook/document' && subject.text.includes(text)), replacement);
        const execution = await readFile(path.join(state, 'body/owner-execution.json'));
        await page.evaluate(view => {
          const button = document.createElement('button'); button.id = 'notebook-cross-mask-stale'; button.textContent = 'Attempt obsolete browser edit';
          button.onclick = async event => {
            try {
              await globalThis.__conduitOwnerParticipation.submitFaceInteraction({ view,
                actionId: 'notebook.replace', target: 'notebook/document', arguments: [{ name: 'text', value: 'Obsolete browser draft' }] });
              globalThis.__notebookCrossStale = { refused: false, trusted: event.isTrusted };
            } catch (error) { globalThis.__notebookCrossStale = { refused: true, trusted: event.isTrusted, code: error.code }; }
          };
          document.querySelector('[data-owner-face]').append(button);
        }, staleView);
        await page.getByRole('button', { name: 'Attempt obsolete browser edit', exact: true }).click();
        await page.waitForFunction(() => globalThis.__notebookCrossStale);
        const stale = await page.evaluate(() => globalThis.__notebookCrossStale);
        assert.equal(stale.refused, true); assert.equal(stale.trusted, true); assert.equal(stale.code, 'StaleShow');
        assert.deepEqual(await readFile(path.join(state, 'body/owner-execution.json')), execution);
        assert.equal(notebook().text, replacement); assert.equal(notebook().revision, 3);
        await writeFile(path.join(evidence, 'proof.json'), JSON.stringify({ schema: 'conduit.notebook/native-mask-proof@1',
          proof_class: 'live-local-qmp-installed-owner', body_id: bodyId, build: buildManifest, provision, qemu_argv: args, cross_mask_stale: stale, policy_evidence: policyEvidence, part, standby, before, ownerBefore, action, after, ownerAfter, notebook: notebook() }, null, 2));
      },
      async close() { socket?.destroy(); for (const child of children.reverse()) await stop(child); await writeFile(path.join(evidence, 'qmp.json'), JSON.stringify(qmpLog, null, 2)); await writeFile(path.join(evidence, 'wardrobe.json'), JSON.stringify(policyEvidence, null, 2)); await writeFile(path.join(evidence, 'processes.json'), JSON.stringify(children.map(child => ({ pid: child.pid, command: child.spawnargs.slice(0, 3), exit_code: child.exitCode, signal: child.signalCode })), null, 2)); },
    };
  } catch (error) {
    socket?.destroy(); for (const child of children.reverse()) await stop(child);
    await writeFile(path.join(evidence, 'qmp.json'), JSON.stringify(qmpLog, null, 2)); await writeFile(path.join(evidence, 'wardrobe.json'), JSON.stringify(policyEvidence, null, 2)); await writeFile(path.join(evidence, 'processes.json'), JSON.stringify(children.map(child => ({ pid: child.pid, command: child.spawnargs.slice(0, 3), exit_code: child.exitCode, signal: child.signalCode })), null, 2)); throw error;
  }
}
