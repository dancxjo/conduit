# Environmental observer joining

The provisioned owner issues one private invitation over the thermometer's USB UART:

```sh
conduit-environment-host --issue-private-invitation OWNER_STATE /dev/ttyUSB0 PRIVATE_BOOTSTRAP.json
```

Keep the bootstrap and its parent directory private (file mode 600, directory mode 700). The invitation expires after five minutes. The public `/body/join` page supplies instructions; it never distributes the bearer secret.

A Linux Host, including a Raspberry Pi running Linux, can consume that invitation:

```sh
conduit-environment-host --join-observer PRIVATE_BOOTSTRAP.json PRIVATE_STATE DEVICE_HOST:80
```

This creates a signed `PRIVATE_STATE/recorder-binding.json` and retains private continuity material. Repeating this command resumes the same admission and its original Boot; it does not create a new Boot. An actual Raspberry Pi run remains a separate physical acceptance requirement.

To return the same admitted Linux Host and part with a fresh explicit Host-process Boot, use a separate output directory:

```sh
conduit-environment-host --return-observer PRIVATE_STATE/recorder-binding.json PRIVATE_STATE/continuity-private.json NEW_PRIVATE_STATE DEVICE_HOST:80
```

The command authenticates the Root's return challenge, signs the canonical part-return proof, verifies the returned membership receipt, and writes `NEW_PRIVATE_STATE/recorder-binding.json`. It preserves the original binding and does not restart an existing recorder. Select the new binding explicitly when starting its replacement. Run this on the original Linux machine: its `/etc/machine-id` must match the admitted Host identity.

Retry with the same output directory after interrupted transport. The retained advertisement preserves the fresh Boot and the signed completion request; read-only recovery can retrieve a committed return whose reply was lost. A completed output directory is idempotent. If an uncommitted challenge expires, the command refuses; start a separately named return attempt rather than editing its retained signed material. A further new Boot also requires a new output directory.

For a browser, generate a private local helper and run the loopback proof proxy on that browser's machine:

```sh
conduit-environment-host --browser-invitation PRIVATE_BOOTSTRAP.json PRIVATE_HELPER.html
conduit-environment-host --browser-join-proxy DEVICE_HOST:80 8787
```

Open the helper locally. Its **Join this Body** and **Reconnect this browser** actions verify Root signatures through WebCrypto. The helper contains the continuity secret; do not publish it. The loopback proxy holds no invitation secret.

These commands admit observation-only Hosts. They do not create an execution offer, Plan, Play, or browser Mask. Joining, recording signed evidence, and installing the Body's durable recorder are separate acceptance boundaries.
