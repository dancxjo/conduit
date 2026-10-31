# Existing CPBR device carrier framing

This crate extracts the already deployed CPBR version-1 framing. It creates no
Conduit Body membership, native possession, network authority or boot identity.
It does not add a new wire version or execute a command.

`FrameDecoder<N>` owns a fixed complete-frame buffer. Its caller supplies the
command policy before payload accumulation. Pico firmware's existing facade
retains its JOIN, BOOTSEL, bench AP and other command meanings and limits.

`InformationSession` and `InformationDecoder<N>` provide one bounded correlated
response to the existing read-only STATUS, SENSOR_STATUS or DIAGNOSTICS commands.
STATUS information is bounded to 512 bytes, sensor/diagnostic information to
1536 bytes, and ERROR to 128 bytes. The adapter must independently validate the
returned role, expected firmware and checked Host configuration. ERROR remains
an ERROR frame; the codec never turns it into valid sensor information.

The session generation is local ownership state, not a new on-wire field.
Request numbers never reset during session advancement. A native owner must
quiesce/drain the actual USB stream before reopening it. No framing property
supplies the missing MCU Boot identity or proves acquisition freshness.
