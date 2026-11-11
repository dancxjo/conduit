/** Failure-only, value-free observation of the existing APT privilege boundary. */
import { spawnSync } from 'node:child_process';

export const PROXY_NAMES = Object.freeze([
  'http_proxy', 'https_proxy', 'all_proxy', 'no_proxy',
  'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'NO_PROXY',
]);
export const REPOSITORY_HOSTS = Object.freeze(['archive.ubuntu.com', 'security.ubuntu.com']);
const DNS_ERROR_CODES = Object.freeze(['EAI_AGAIN', 'EAI_FAIL', 'EAI_SYSTEM', 'ENOTFOUND', 'EACCES', 'EPERM', 'ENETUNREACH', 'ETIMEDOUT']);
const MAXIMUM_OUTPUT = 4096;
const TIMEOUT_MS = 3000;

// This is the same UID/sudo selection as acquisition, without preserving extra
// environment or changing sudo policy. The probe itself uses one fixed Node;
// command-specific sudoers rules could still differ from the apt-get executable.
export function privilegedCommand(program, args, uid = process.getuid?.()) {
  return uid === 0 ? { program, args } : { program: 'sudo', args: [program, ...args] };
}

// No command, path, environment value or error message is accepted from a caller.
// Raw apt-config output stays inside this bounded child and is never forwarded.
const PROBE = `
const fs = require('node:fs');
const crypto = require('node:crypto');
const {spawnSync} = require('node:child_process');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const proxy = Object.fromEntries(${JSON.stringify(PROXY_NAMES)}.map(name => [name,
  Object.prototype.hasOwnProperty.call(process.env, name)]));
let resolver = {status:'unavailable'};
let fd;
try {
  const entry = fs.lstatSync('/etc/resolv.conf');
  resolver = {status:'unavailable', symlink:entry.isSymbolicLink(),
    entryMode:(entry.mode & 0o7777).toString(8).padStart(4,'0')};
  if (entry.isSymbolicLink()) resolver.linkSha256 = hash(fs.readlinkSync('/etc/resolv.conf'));
  fd = fs.openSync('/etc/resolv.conf', fs.constants.O_RDONLY | fs.constants.O_NONBLOCK);
  const file = fs.fstatSync(fd);
  resolver.targetMode = (file.mode & 0o7777).toString(8).padStart(4,'0');
  if (!file.isFile() || file.size > 16384) resolver.status = 'limit';
  else {
    const bytes = Buffer.alloc(16385);
    const count = fs.readSync(fd, bytes, 0, bytes.length, 0);
    if (count > 16384) resolver.status = 'limit';
    else { resolver.status = 'ok'; resolver.contentSha256 = hash(bytes.subarray(0,count)); }
  }
} catch {} finally { if (fd !== undefined) { try { fs.closeSync(fd); } catch {} } }
let aptProxy = {status:'unavailable'};
try {
  const result = spawnSync('/usr/bin/apt-config', ['dump'], {encoding:'utf8',
    timeout:1000, maxBuffer:65536, killSignal:'SIGKILL'});
  if (!result.error && result.status === 0) {
    const keys = String(result.stdout).split('\\n').map(line => line.trim().split(/\\s/)[0].toLowerCase());
    const present = key => keys.some(actual => actual === key.toLowerCase() || actual.startsWith(key.toLowerCase()+'::'));
    aptProxy = {status:'ok', http:present('Acquire::http::Proxy'),
      https:present('Acquire::https::Proxy'),
      httpAutoDetect:present('Acquire::http::Proxy-Auto-Detect') || present('Acquire::http::ProxyAutoDetect'),
      httpsAutoDetect:present('Acquire::https::Proxy-Auto-Detect') || present('Acquire::https::ProxyAutoDetect')};
  }
} catch {}
// One OS-resolver lookup per already-approved host, never a mirror or retry.
const dns = require('node:dns');
const hosts = ${JSON.stringify(REPOSITORY_HOSTS)};
const codes = ${JSON.stringify(DNS_ERROR_CODES)};
const observation = {proxy,resolver,aptProxy,lookup:{}};
let closed = false;
let pending = hosts.length;
function publish() {
  if (closed) return;
  closed = true;
  clearTimeout(deadline);
  process.stdout.write(JSON.stringify(observation), () => process.exit(0));
}
const deadline = setTimeout(() => {
  for (const host of hosts) if (!Object.hasOwn(observation.lookup,host))
    observation.lookup[host] = {status:'timeout'};
  publish();
},1000);
for (const host of hosts) {
  const complete = (error,addresses) => {
    if (closed || Object.hasOwn(observation.lookup,host)) return;
    if (error) observation.lookup[host] = {status:'failed',code:codes.includes(error.code) ? error.code : 'other'};
    else {
      const ipv4 = Array.isArray(addresses) && addresses.some(item => item?.family === 4);
      const ipv6 = Array.isArray(addresses) && addresses.some(item => item?.family === 6);
      observation.lookup[host] = ipv4 || ipv6 ? {status:'resolved',ipv4,ipv6} : {status:'unavailable'};
    }
    if (--pending === 0) publish();
  };
  try { dns.lookup(host,{all:true},complete); } catch (error) { complete(error); }
}

`;

function safeProbe(result) {
  if (result?.error || result?.status !== 0 || typeof result?.stdout !== 'string'
      || Buffer.byteLength(result.stdout) > MAXIMUM_OUTPUT) return { status: 'unavailable' };
  try {
    const value = JSON.parse(result.stdout);
    const proxy = Object.fromEntries(PROXY_NAMES.map(name => {
      if (typeof value.proxy?.[name] !== 'boolean') throw new Error();
      return [name, value.proxy[name]];
    }));
    const resolver = {};
    if (!['ok', 'unavailable', 'limit'].includes(value.resolver?.status)) throw new Error();
    resolver.status = value.resolver.status;
    for (const key of ['entryMode', 'targetMode']) if (value.resolver[key] !== undefined) {
      if (typeof value.resolver[key] !== 'string' || !/^[0-7]{4}$/.test(value.resolver[key])) throw new Error();
      resolver[key] = value.resolver[key];
    }
    if (value.resolver.symlink !== undefined) {
      if (typeof value.resolver.symlink !== 'boolean') throw new Error();
      resolver.symlink = value.resolver.symlink;
    }
    for (const key of ['linkSha256', 'contentSha256']) if (value.resolver[key] !== undefined) {
      if (typeof value.resolver[key] !== 'string' || !/^[a-f0-9]{64}$/.test(value.resolver[key])) throw new Error();
      resolver[key] = value.resolver[key];
    }
    const aptProxy = {};
    if (!['ok', 'unavailable'].includes(value.aptProxy?.status)) throw new Error();
    aptProxy.status = value.aptProxy.status;
    if (aptProxy.status === 'ok') for (const key of ['http', 'https', 'httpAutoDetect', 'httpsAutoDetect']) {
      if (typeof value.aptProxy[key] !== 'boolean') throw new Error();
      aptProxy[key] = value.aptProxy[key];
    }
    const lookup = Object.fromEntries(REPOSITORY_HOSTS.map(host => {
      const entry = value.lookup?.[host];
      let safe = {status:'unavailable'};
      if (entry?.status === 'timeout') safe = {status:'timeout'};
      else if (entry?.status === 'failed') safe = {status:'failed',
        code:DNS_ERROR_CODES.includes(entry.code) ? entry.code : 'other'};
      else if (entry?.status === 'resolved' && typeof entry.ipv4 === 'boolean'
          && typeof entry.ipv6 === 'boolean' && (entry.ipv4 || entry.ipv6))
        safe = {status:'resolved',ipv4:entry.ipv4,ipv6:entry.ipv6};
      return [host,safe];
    }));
    return { status: 'ok', proxy, resolver, aptProxy, lookup };
  } catch { return { status: 'unavailable' }; }
}

// Do not use acquisition's execute/error formatter: it includes raw stderr on
// failure. Every failure here becomes a fixed status, never captured text.
export function emitAptFailureDiagnostics({ spawn = spawnSync, uid = process.getuid?.(),
  report = line => console.error(line) } = {}) {
  const node = process.execPath;
  const args = ['-e', PROBE];
  const invoke = command => {
    try {
      return safeProbe(spawn(command.program, command.args, {encoding:'utf8',
        timeout:TIMEOUT_MS, maxBuffer:MAXIMUM_OUTPUT, killSignal:'SIGKILL',
        env:{...process.env,LC_ALL:'C'}}));
    } catch { return {status:'unavailable'}; }
  };
  const observation = {schema:'conduit.ci/apt-failure-diagnostics@2', alreadyRoot:uid === 0,
    unprivileged:invoke({program:node,args}),
    privileged:invoke(privilegedCommand(node,args,uid))};
  // Reporter failures likewise have no authority to replace the APT refusal.
  try { report(JSON.stringify(observation)); } catch {}
  return observation;
}
