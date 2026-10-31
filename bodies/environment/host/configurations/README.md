# Instrument placement

Edit `metadata = {placement: "Forebrain room"}` in the corresponding
`green.host.conduit`, `white.host.conduit` or `yellow.host.conduit` file. Use a
nonempty description of at most 96 UTF-8 bytes, without control characters.

Check the edited source through the ordinary Make entrance:

```sh
cargo +stable xtask make host config check bodies/environment/host/configurations/green.host.conduit
```

Placement is descriptive Host context, not equipment configuration or authority.
It preserves the qualified hardware realization identity and does not require a
firmware reflash. The collection owner must load the edited source before it can
publish updated placement; editing a file does not itself modify a running Face.
Old observations retain their source/session identity. Current placement does
not establish where past samples were acquired.

These files preserve their qualified profile's Host declaration and all hardware,
software and resource selections. `Green`, `White` and `Yellow` are instrument
labels; renaming the underlying Host declaration would change its realization
identity.

Blue's location is pending identification of its actual qualified native Host
source configuration. A speculative replacement profile could invalidate its
existing firmware/Body pins, so no Blue file is fabricated here.
