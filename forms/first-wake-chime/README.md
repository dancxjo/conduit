# First wake Chime

This canonical non-graphical form connects `body/first-wake` to the same
`sound/startup-chime` sink as the ordinary startup example. Its explicit scope
is the body lifetime: it runs in the first play of the first wake, and remains
silent on later wake/lull/wake cycles and reloads of that retained body.

Choose First wake Chime in the shared Crèche reached by
`cargo xtask demo workspace`. Omit Startup Chime if you want only the first-wake
behavior. See [Startup Chime](../startup-chime/README.md) for exact persistence,
rebirth, optional-audio, and proof boundaries.
