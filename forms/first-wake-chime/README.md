# First Wake Chime

This canonical non-graphical Form connects `body/first-wake` to the same
`sound/startup-chime` sink as the ordinary startup example. Its explicit scope
is the Body lifetime: it runs in the first Play of the first Wake, and remains
silent on later wake/lull/wake cycles and reloads of that retained Body.

Choose First Wake Chime in the shared Crèche reached by
`cargo xtask demo workspace`. Omit Startup Chime if you want only the first-wake
behavior. See [Startup Chime](../startup-chime/README.md) for exact persistence,
rebirth, optional-audio, and proof boundaries.
