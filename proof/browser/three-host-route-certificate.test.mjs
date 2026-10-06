import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { verifyGuestRouteCertificate } from './three-host-route-certificate.mjs';

test('guest route preflight rejects an IP SAN with the wrong common name', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'conduit-route-cert-'));
  try {
    const key = path.join(root, 'route.key');
    const cert = path.join(root, 'route.crt');
    execFileSync('openssl', ['ecparam', '-name', 'prime256v1', '-genkey', '-noout', '-out', key]);
    const sign = commonName => execFileSync('openssl', ['req', '-new', '-x509', '-sha256',
      '-days', '1', '-key', key, '-out', cert, '-subj', `/CN=${commonName}`,
      '-addext', 'subjectAltName=IP:10.0.2.42,IP:192.168.1.194']);
    sign('conduit-local-proof');
    assert.throws(() => verifyGuestRouteCertificate(readFileSync(cert), readFileSync(key),
      'wss://10.0.2.42:4433/conduit'), /common name or DNS SAN/);
    sign('10.0.2.42');
    assert.equal(verifyGuestRouteCertificate(readFileSync(cert), readFileSync(key),
      'wss://10.0.2.42:4433/conduit').subject, 'CN=10.0.2.42');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
