# Fore firmware continuation and reconnect boundary

Read-only audit of current shared Source composition on 2026-11-16. No firmware/device/Root changes were made by this audit.

The carrier-enabled firmware can cold-resume local admitted Plays: `runtime_continuation_factory.rs::prepare_into` explicitly passes `line=None`, constructing `BoundSampling::Local`; `runtime.rs` can then lawfully call `into_local`. Resume still requires matching signed Source, image, configuration and owner pins, original finite grants, complete admission and fresh-Boot Start. Old continuation bytes do not qualify the new configuration.

`play_carriage.rs::Io::run` permanently retires the bridge on the first online Line pump error, retaining the exclusive transport loan and undelivered Values while independent native runners continue. `play_fore_installation.rs` advertises `LineContinuation::None`. Cold local continuation serves inspection and membership-only Return; it does not attach a history Fore to existing Started kernels. Online Line mode has no concurrent Root control channel. Removing the `into_local` refusal or resetting the session state is not a lawful reconnect implementation.

## Deployment evidence still required

1. First admitted record read with the newly checked firmware and exact current Root policy.
2. Carrier interruption with a read result outstanding: local sensing/display continue, result custody retained, no invented delivery acknowledgment.
3. Physical power-cycle local continuation with the new exact signed pins.
4. A separately authorized, finitely admitted Line replacement/attachment transition preserving the existing execution, undelivered Values and peer/Boot scope before claiming reconnecting history.

This is a remaining Line/lifetime integration obligation, not a new memory epic. It must not be conflated with successful first delivery or local restart continuity.
