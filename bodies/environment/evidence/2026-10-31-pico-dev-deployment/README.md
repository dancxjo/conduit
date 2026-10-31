# Current-dev Pico deployment

Sources include dev `1500a17c3d`, merged in `bafaac1572`, with both independent
focused-suite dispatch additions retained and formatted in `051d030038`.
No experimental semantic-law changes are included.

Green's current-dev rebuild is byte-identical to its running, previously
qualified credential-store image. Its observed 2,243 readings and 33,636 display
ACKs were fault-free, with restored credential revision 1 and zero commits this
boot. No redundant reflash was performed.

White on beeplus was identified through its unowned USB stream before updating.
The existing project headless-mount installer was installed with administrator
authentication; its installed helper hash matched the reviewed project source.
Only contiguous application UF2 pages were written, with credential and bootstrap
sectors excluded. The old application recovery UF2 was retained and verified as
`bf35fe403cc842081624cfc0b1c2c5f53fd8a9bdb29a907ea95832ad233a8ce4`.
White's physical USB serial is now `e6639c758f78952c`.

White's actual JEDEC ID matched the credential-store profile. The existing
canonical JOIN wire provisioned the authorized network over private SSH stdin,
without copying a network credentials file to beeplus. One verified credential
commit took 30,465 us; this does not replace the reviewed timing envelope. A
second application-only load of the same image restarted White. No subsequent
JOIN was sent. It restored revision 1 with zero commits, automatically obtained
`192.168.1.248`, and served two readings/18 display ACKs without reported faults.
This is a firmware restart test, not an independently observed cold power cycle.

Yellow on motherbrain was initially observed measuring successfully. Its new
image builds and passes the shared UF2 parser, but is **not deployed**: the Pi
briefly returned, then both hostname discovery and its last address became
unavailable. The old application remains selected; no Yellow reset was sent.
The retained old Yellow recovery UF2 SHA256 is
`69a63f9b6a6994129d7102358478abe0c6cd991bdf1fb1520154b82b203acd3f`.

All three images were built on forebrain, without compiling on the Pis:

```
CONDUIT_HOST_CONFIGURATION=<absolute checked profile> \
CARGO_TARGET_DIR=<private build output> CARGO_BUILD_JOBS=3 \
cargo +stable build --locked --offline --release \
  --target thumbv6m-none-eabi \
  --manifest-path targets/rp2040/proof/environment-usb/Cargo.toml \
  --features <exact features>
```

| Device | Profile under targets/rp2040/profiles | Features |
| --- | --- | --- |
| Green | pico-w-environment-usb.host.conduit | credential-store |
| White | pico-w-bme280-tm1637.host.conduit | credential-store,bosch-tm1637 |
| Yellow | pico-w-dht11-tm1637.host.conduit | credential-store,dht11-tm1637 |

`proof.json` records exact firmware and artifact hashes, unchanged checked
configuration identities, and bounded UF2 parser results. Application-only
address validation is not a full retained-flash readback. Display ACKs are not
optical acceptance. These are diagnostic carriers (`body_member:false`), not
newly admitted Body Hosts, durable observation recorders or protected Lines.
No Linux Host/display/history services were changed by this deployment.
