# Operate a private Conduit rendezvous relay

The user-operated relay gives two intended Hosts an outbound-only rendezvous
path when neither can accept an inbound connection. It forwards finite opaque
protected frames for one pre-provisioned pair. It is not a directory, durable
mailbox, Body membership service, planner, or source of endpoint authority.

This path needs a publicly reachable host, a DNS name, and a WebPKI-valid TLS
certificate and private key for that name. The protocol has no cloud-vendor
dependency. Keep the relay slot, both endpoint descriptors, and TLS private key
private.

## Provision one pair

Obtain the SHA-256 digest of the exact leaf certificate using the tooling from
your certificate provider. Then create a new private directory containing one
relay slot and two role-specific endpoint descriptors:

```console
conduit rendezvous-relay provision \
  --relay-address 203.0.113.10:7443 \
  --relay-url wss://relay.example:7443/conduit \
  --server-identity relay.example \
  --certificate-sha256 <64-hex-leaf-certificate-digest> \
  --first-host-id <first-host-id> \
  --first-boot-id <first-boot-id> \
  --second-host-id <second-host-id> \
  --second-boot-id <second-boot-id> \
  --output private-relay \
  --authorize-provision
```

Provisioning refuses to overwrite an existing path. On Unix it creates the
directory with mode `0700` and each JSON file with mode `0600`. The default
candidate expires after ten minutes and allows one attempt; those finite bounds
may be reduced or changed within the command's admitted ranges. Provision again
for a new attempt rather than editing or recycling expired credentials.

The endpoint file is a private deployment wrapper, not a second rendezvous
protocol. Its `rendezvous` value is the same bounded, versioned base64url
manifestation of the canonical CBOR/CDDL descriptor consumed by browser, std,
and ConduitOS. Relay attachment and protected-session metadata remain separate;
the host refuses the file if either relabels the shared candidate.

Give `endpoint-first.json` only to the first intended Host and
`endpoint-second.json` only to the second. Transfer them over an already trusted
private channel. The files contain bearer capabilities and end-to-end session
key material; do not paste them into issues, logs, or evidence.

## Run the relay

On the routable relay host, start the exact pre-provisioned slot:

```console
conduit rendezvous-relay serve \
  --bind 203.0.113.10:7443 \
  --public-url wss://relay.example:7443/conduit \
  --tls-cert relay-chain.pem \
  --tls-key relay-key.pem \
  --slot private-relay/relay-slot.json \
  --authorize-network
```

The public URL, listener, certificate identity, and slot must agree. The relay
accepts exactly the two authorized outbound attachments, keeps no pre-pair or
offline mailbox, refuses excess pressure, and stops on expiry, explicit close,
loss, or its finite lifetime. Its bounded terminal evidence contains identities,
counters, and dispositions, but not capabilities, keys, or ordinary session
payloads.

## Connect the two Hosts

On each intended native Host, use its own descriptor:

```console
conduit host rendezvous \
  --state-dir <installed-host-state> \
  --carrier relay \
  --relay-descriptor private-relay/endpoint-first.json
```

Run the corresponding command with `endpoint-second.json` on the other Host.
Both connections are outbound. Relay TLS authenticates and protects each outer
connection; the separate protected Line authenticates the exact peer/session
and keeps ordinary Conduit frames opaque to the relay. The normal invitation,
Join, and Body-admission protocol still runs above that Line and grants no
authority to the relay.

There is no hidden reconnect loop. Unreachable relay, TLS authentication,
expiry, binding refusal, pressure, end-to-end authentication, active loss, and
ordinary Line loss remain distinct. Any next candidate is selected only by the
existing finite rendezvous schedule.

## Current proof boundary

Repository tests exercise a real TLS relay with two outbound native clients,
end-to-end protected traffic, the ordinary host rendezvous session, bounded
pressure/loss, and finite fallback scheduling. Pinned Chromium exercises the
same candidate contract and browser protected-frame adapter. ConduitOS consumes
the portable candidate contract but truthfully refuses execution until its
entropy, Noise, TLS, and bounded socket realization is admitted.

These checks do not substitute for the open three-machine, two-NAT physical
acceptance run in [issue #3621](https://github.com/dancxjo/conduit/issues/3621).
They also do not prove that a browser can inspect a TLS leaf fingerprint: the
browser relies on WebPKI for the outer connection while the expected digest is
bound into the end-to-end session identity.
