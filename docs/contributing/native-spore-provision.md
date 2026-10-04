# Provision a ConduitOS guest from a running Body owner

This development entrance binds one verified x86_64 ConduitOS product ISO to
one checked, self-joining Body Host and a fresh invitation issued by the
running Linux Body owner. The output boots into a **pending join**. Provisioning
alone does not admit the guest, establish a protected Line, or return the owner's receipt.
Because the image contains the invitation secret, this producer is available
on Unix hosts where it can create private media with mode `0600`.

First build the product image and keep its `build-manifest.json` and
`resolved-image.json` beside the ISO:

```sh
cargo xtask make conduitos live x86_64
```

Use the installed owner described in [Local Linux Body owner](shared-body-owner.md)
to issue a short-lived invitation through its `invite` operation. Save the
single `conduit.body/spawn-invitation@1` JSON document as a private file. It
contains a one-use secret; do not put it in a repository, a command argument,
or a journey capture. The owner must retain its issued invitation and remain
available to consume the guest's later admission request.

Author a `.body.conduit` source with the **same Body ID and invitation ID** as
that owner document. Its ConduitOS Host entry must select `self-joining` and
`disk-image`, with the reviewed
`targets/conduitos/profiles/conduitos-native.host.conduit` configuration. For
example, with paths and IDs filled from the actual owner and checkout:

```conduit
body shared {
  schema = 2
  id = "<owner-body-id>"
  host = {name: "native", configuration: "<path-to-conduitos-native.host.conduit>", spore: {join_mode: "self-joining", invitation: "<owner-invitation-id>", output: "disk-image"}}
}
```

Then use the supported checks and producer:

```sh
cargo xtask make body check path/to/shared.body.conduit
cargo xtask make body provision-conduitos path/to/shared.body.conduit \
  --host native \
  --build target/conduitos/live/x86_64-pc \
  --invitation path/to/private-invitation.json \
  --output path/to/new-private-spore.iso
cargo xtask make conduitos acceptance --spore path/to/new-private-spore.iso
```

The producer verifies the exact target BUILD and ISO digest, uses the checked
Body/make binding to seal the actual ISO content digest, and replaces exactly
one blank 32 KiB media region. It refuses an existing output, ambiguous or
already provisioned media, a mismatched Body or invitation, and an expired
invitation. The created ISO has mode `0600` on Unix. Its receipt records the
source identity and exact image/artifact digests without serializing the secret.

The foreground owner can issue an unrouted invitation. When an owner
issues a canonical routed invitation, the producer retains its exact ordered
candidates and transport authentication binding; the guest decoder and QEMU
acceptance validator refuse a relabeled or weakened descriptor. The boot path
keeps the public route and certificate pins in pending state, reports its
candidate count in the Face, and emits a signed serial observation. With a
supported network and calibrated deadline clock it attempts one bounded,
authenticated owner exchange and installs only the owner's verified receipt.
For each distinct TLS leaf named by a routed invitation, also pass
`--route-tls-cert path/to/owner-certificate.pem` to the producer. It retains
only the first public certificate from that PEM file, checks its SHA-256 against
the exact candidate binding, and refuses a missing, extra, duplicate, or
oversized certificate. Unrouted invitations require no certificate.
The acceptance command proves a QEMU boot and the invitation-signed serial
observation only; its receipt does not claim guest membership. With a reachable
pinned TLS owner listener, the native guest now sends the signed request,
verifies and installs the owner's canonical admission receipt, and uses the
owner-sealed Mask route for a live Face, acknowledged Show, and typed clock
action. `cargo xtask make conduitos live-owner-action-proof` exercises that
path. The [three-Host proof](native-three-host-proof.md) keeps the guest and
browser live together under one installed owner and captures their shared
Face and actions. These are local development proofs, not accepted release or
public journey evidence. For #4807 publication, build and capture from one
clean source revision and record the owner, model, and voice inputs separately.
