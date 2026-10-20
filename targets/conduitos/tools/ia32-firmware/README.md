# Reviewed IA-32 firmware pin (#6057, preparation evidence)

`manifest.json` records the ordinary (non-secure-boot) OVMF32 CODE/VARS pair
from Ubuntu's `ovmf-ia32` binary package. Explicit preparation and local verification enter through:

```sh
cargo xtask make conduitos prepare-ia32-firmware
cargo xtask make conduitos doctor-ia32-firmware
```

Setup needs `curl` and `dpkg-deb` only if no matching installed pair or verified
repository cache exists. Ubuntu 24.04's installed matching pair is preferred;
Ubuntu 26.04 needs no removed `ovmf-ia32` APT package. CI uses the same command.
A corrupt/incomplete repository cache refuses rather than being silently
replaced: preserve it for diagnosis, move it aside, then rerun preparation.
Doctor and `live-boot ia32` verify local bytes and never download. Each UEFI
run copies the immutable VARS template into its own writable variable store.
These checks establish acquisition, not a ConduitOS boot or graphics result.
The package is extracted as data, never installed with dpkg or mixed into
the host's APT sources. `copyright` preserves upstream/component licensing with trailing whitespace normalized.

On 19 October 2026 the retained `noble-updates.InRelease` was verified with
`gpgv` against `/usr/share/keyrings/ubuntu-archive-keyring.gpg`, producing a
good signature from Ubuntu Archive Automatic Signing Key (2018), fingerprint
`F6ECB3762474EDA9D21B7022871920D1991BC93C`. Its SHA256 section independently
pins `universe/binary-amd64/Packages.xz` to the hash and size in the manifest.
The exact authenticated index remains addressable through the archive's
`dists/noble-updates/universe/binary-amd64/by-hash/SHA256/` endpoint plus that
hash. `package.stanza` is the package entry extracted from that verified index.
The downloaded package matched the index's SHA256 and size before extraction;
the extracted CODE/VARS hashes and lengths are recorded independently.

This retained release signature is provenance for the fixed pin, not permission
to trust arbitrary later archive metadata, bypass expiry in APT, install a
cross-release package, or download during boot. The eventual explicit xtask
setup must verify the pinned package and both extracted files, preserve an
immutable VARS template, stage atomically, and honor a verified offline cache.
Doctor and execution must perform local checks without downloading.
