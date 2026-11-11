import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { runInNewContext } from 'node:vm';
import { emitAptFailureDiagnostics, privilegedCommand, PROXY_NAMES } from '../../tools/ci/pipeline/acquisition/apt-failure-diagnostics.mjs';

const secret = 'never-print-this-proxy-credential';
const resolver = Buffer.from('nameserver 192.0.2.53\n');
const hash = value => createHash('sha256').update(value).digest('hex');
function child(script, env, aptResult = {}) {
  let stdout = '';
  const fs = {
    constants: {O_RDONLY:0,O_NONBLOCK:2048},
    lstatSync(file) { assert.equal(file,'/etc/resolv.conf'); return {mode:0o120777,isSymbolicLink:() => true}; },
    readlinkSync() { return `/private/${secret}`; },
    openSync(file, flags) { assert.equal(file,'/etc/resolv.conf'); assert.equal(flags,2048); return 7; },
    fstatSync() { return {mode:0o100644,size:resolver.length,isFile:() => true}; },
    readSync(fd, bytes) { assert.equal(fd,7); resolver.copy(bytes); return resolver.length; },
    closeSync(fd) { assert.equal(fd,7); },
  };
  runInNewContext(script, {
    Buffer,
    process:{env,stdout:{write(value) {stdout += value;}}},
    require(name) {
      if (name === 'node:fs') return fs;
      if (name === 'node:crypto') return {createHash};
      assert.equal(name,'node:child_process');
      return {spawnSync(program,args,options) {
        assert.equal(program,'/usr/bin/apt-config'); assert.deepEqual([...args],['dump']);
        assert.equal(options.timeout,1000); assert.equal(options.maxBuffer,65536);
        return {status:0,stdout:`Acquire::http::Proxy "https://${secret}@proxy.invalid";\nAcquire::https::ProxyAutoDetect "${secret}";\n`,stderr:secret,...aptResult};
      }};
    },
  }, {timeout:100});
  return {status:0,stdout,stderr:secret};
}

test('fixed probes compare the actual sudo boundary without disclosing values', () => {
  const calls = []; const lines = [];
  const observed = emitAptFailureDiagnostics({uid:1000,report:line => lines.push(line),
    spawn(program,args,options) {
      calls.push([program,args,options]);
      assert.equal(options.timeout,3000); assert.equal(options.maxBuffer,4096);
      assert.equal(options.killSignal,'SIGKILL');
      assert.equal(options.env?.LC_ALL,'C','match ordinary acquisition locale without exposing other environment values');
      const script = program === 'sudo' ? args[2] : args[1];
      return child(script, program === 'sudo' ? {} : {https_proxy:`https://${secret}`,UNRELATED_SECRET:secret});
    }});
  assert.equal(calls[0][0],process.execPath);
  assert.deepEqual(calls[0][1].slice(0,1),['-e']);
  assert.equal(calls[1][0],'sudo');
  assert.deepEqual(calls[1][1].slice(0,2),[process.execPath,'-e']);
  assert.equal(calls[0][1][1],calls[1][1][2]);
  assert.equal(observed.unprivileged.proxy.https_proxy,true);
  assert.equal(observed.privileged.proxy.https_proxy,false);
  assert.deepEqual(Object.keys(observed.unprivileged.proxy),[...PROXY_NAMES]);
  assert.deepEqual(observed.unprivileged.resolver, {status:'ok',entryMode:'0777',targetMode:'0644',
    symlink:true,linkSha256:hash(`/private/${secret}`),contentSha256:hash(resolver)});
  assert.deepEqual(observed.unprivileged.aptProxy,
    {status:'ok',http:true,https:false,httpAutoDetect:false,httpsAutoDetect:true});
  assert.equal(lines.length,1);
  assert.doesNotMatch(lines[0],new RegExp(`${secret}|proxy\\.invalid|nameserver|192\\.0\\.2\\.53|https://|UNRELATED_SECRET`));
});

test('root path stays root and does not add a new privilege policy', () => {
  const calls = [];
  const observed = emitAptFailureDiagnostics({uid:0,report() {},spawn(program,args) {
    calls.push(program); return child(args[1],{});
  }});
  assert.equal(observed.alreadyRoot,true);
  assert.deepEqual(calls,[process.execPath,process.execPath]);
  assert.deepEqual(privilegedCommand('apt-get',['update'],1000),{program:'sudo',args:['apt-get','update']});
  assert.deepEqual(privilegedCommand('apt-get',['update'],0),{program:'apt-get',args:['update']});
});

for (const result of [
  {status:1,stdout:secret,stderr:secret},
  {status:null,error:{code:'ETIMEDOUT',message:secret},stdout:secret,stderr:secret},
  {status:0,stdout:'x'.repeat(4097)},
  {status:0,stdout:JSON.stringify({proxy:{http_proxy:secret},resolver:{status:secret},aptProxy:{status:secret}})},
]) test('failed or invalid captured output is never an error-formatting input', () => {
  const lines = [];
  const observed = emitAptFailureDiagnostics({uid:1000,spawn:() => result,report:line => lines.push(line)});
  assert.deepEqual(observed.unprivileged,{status:'unavailable'});
  assert.deepEqual(observed.privileged,{status:'unavailable'});
  assert.doesNotMatch(lines.join(''),new RegExp(secret));
});

test('spawn and reporter exceptions remain diagnostic-only', () => {
  assert.doesNotThrow(() => emitAptFailureDiagnostics({spawn() {throw new Error(secret);},
    report() {throw new Error(secret);}}));
});

test('apt-config failure never forwards its captured output', () => {
  const lines = [];
  const observed = emitAptFailureDiagnostics({uid:1000,report:line => lines.push(line),
    spawn(program,args) {return child(program === 'sudo' ? args[2] : args[1],{},
      {status:1,stdout:secret,stderr:secret,error:{message:secret}});}});
  assert.deepEqual(observed.unprivileged.aptProxy,{status:'unavailable'});
  assert.doesNotMatch(lines.join(''),new RegExp(secret));
});

test('unknown fields cannot smuggle values through an otherwise valid summary', () => {
  const lines = [];
  const raw = {status:0,stdout:JSON.stringify({proxy:Object.fromEntries(PROXY_NAMES.map(name => [name,false])),
    resolver:{status:'unavailable',unexpected:secret},aptProxy:{status:'unavailable',value:secret},
    fullEnvironment:{CREDENTIAL:secret}})};
  const observed = emitAptFailureDiagnostics({spawn:() => raw,report:line => lines.push(line)});
  assert.equal(observed.unprivileged.status,'ok');
  assert.doesNotMatch(lines.join(''),new RegExp(`${secret}|fullEnvironment|CREDENTIAL|unexpected`));
});
