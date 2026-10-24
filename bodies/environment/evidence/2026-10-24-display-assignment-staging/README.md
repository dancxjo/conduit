# Physical native output assignment staging

Ran `cargo xtask prove environment display-output esp32-readiness --flash --stage`
on the ESP32 identified in `staging.json`, from the clean source and image pinned
in `image.json`. The host planned against current observed native/portable offers
and sent the resulting 2,427-byte bounded output assignment transfer.

Root refused changed Source identity, changed Boot identity and changed canonical
payload digest without retaining an assignment seal. The valid transfer staged
successfully with exact Plan/fragment hashes and wire seal. Duplicate staging
refused without replacing the retained seal. Read-only offers correctly reported
display assignment custody; actual native availability remained unchanged.

After staging, Root used 66,956 bytes with a 96,984-byte Boot peak. It reported
47,732 bytes free and a 38,440-byte additional kernel-preparation bound. That
estimate fits the observed staging point; it is not an executed allocation,
native binding/context admission or proof of the complete live memory closure.
No native possession was issued and no Body or Plan was installed or started.

The scripted final step restored the integrity-checked working image. An eighteen
second recovery capture contained six current/fresh display updates at advancing
source sequences 7 and 8, all acknowledged by the driver. Human visual output and
HTTP reachability were not checked in this capture.

Raw replies, transfer and image remain in ignored
`target/environment-display-readiness/1792828502`; restoration logs/readings are
in `target/environment-display-stage-probe/1792828476`. Small committed receipts
retain their hashes. Native preparation, lifecycle commands, task registration,
Frame delivery, the live Mask client, sensing/network coexistence, recording and
overnight endurance remain separate proof. The working firmware was restored.
