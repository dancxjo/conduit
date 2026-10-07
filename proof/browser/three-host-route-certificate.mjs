import assert from 'node:assert/strict';
import { createPrivateKey, createPublicKey, X509Certificate } from 'node:crypto';

// The current guest's pinned verifier accepts DNS SANs and a common name for
// hostname matching. An IP SAN alone is insufficient for that guest even
// though it is required by ordinary IP-address TLS clients.
export function verifyGuestRouteCertificate(certificatePem, keyPem, routeUrl) {
  const route = new URL(routeUrl);
  assert.equal(route.protocol, 'wss:', 'the guest route must use WSS');
  assert.equal(route.pathname, '/conduit', 'the guest route must use the admitted path');
  const certificate = new X509Certificate(certificatePem);
  const guestHost = route.hostname;
  assert.equal(certificate.checkIP(guestHost), guestHost,
    'route certificate must carry the guest IP in its IP SAN');
  const commonName = certificate.subject.split('\n').find(field => field.startsWith('CN='))?.slice(3);
  const dnsNames = certificate.subjectAltName?.split(/,\s*/)
    .filter(name => name.startsWith('DNS:')).map(name => name.slice(4)) ?? [];
  assert.ok(commonName === guestHost || dnsNames.includes(guestHost),
    'guest verifier requires the route IP as common name or DNS SAN');
  assert.ok(['ec', 'ed25519'].includes(certificate.publicKey.asymmetricKeyType),
    'guest TLS requires an ECDSA or Ed25519 route key');
  assert.deepEqual(
    createPublicKey(createPrivateKey(keyPem)).export({ type: 'spki', format: 'der' }),
    certificate.publicKey.export({ type: 'spki', format: 'der' }),
    'route key does not match its certificate',
  );
  return certificate;
}
