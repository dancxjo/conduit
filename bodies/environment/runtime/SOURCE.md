# Checked DHT11 Source conversion

`observation::PreparedSourceFrameDecoder` prepares the ordinary portable
`dht11-decode` expression and existing observation quantity Forms once, then
converts a captured length and five padded octets without allocating. Source
owns checksum, length, range, sign and fractional interpretation. Results retain
first-class temperature and relative humidity, or typed unavailability. A bad
frame never becomes a fabricated zero observation.

The build helper checks the authored pulse types, DHT11 decoder and capture
request, and embeds the resulting portable programs with their Source, checked
and expanded identities. These identities describe meaning; they are not Body,
Host, Plan, Play, observation provenance or effect authority.

With the optional `source-capture` feature, `capture_request` evaluates the
checked request expression and validates its electrical geometry against supplied
pulse limits. The returned request is data. It initializes no GPIO provider,
selects no host and grants no permission to capture.

Run `cargo xtask prove environment source` for default and source-capture tests
and strict lint. The proof replays retained physical octets through real Source,
checks invalid and recovered results, negative values and canonical quantities,
and projects one result into the existing Face and Fahrenheit Mask. It checks
zero allocations during conversion. It is CPU conformance, not a fresh physical
capture, installed native appliance or overnight endurance proof.

Tracked by #6376, as a prerequisite of #6295. Native scheduling, selected provider
custody, continuous Face/display execution, network transition and durable
recording remain separate installation and integration work.
