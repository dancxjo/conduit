# Optional network attachment capability

The first network capability slice keeps five facts distinct:

```text
finite Wi-Fi station resource
  -> optional equal-face network/join capability
  -> admitted provider host operation under exact authority
  -> boot-scoped NetworkAttachment runtime fact
  != WebSocket provider or Conduit link
```

`conduit-net` is a `no_std` semantic/provider contract, not a `conduit.std`
catalog family and not mandatory host core. A concrete host advertises the
`conduit.resource/network/wifi-station@1` resource only when its selected
composition provides one. A separately selected `network/join` offer consumes
exactly one such resource and one network-configuration authority grant.
Callable compatibility is canonical checked-face equality; resource, selected
capability, authority, host, and boot identities remain exact admission gates.

The first request carries bounded volatile SSID and credential bytes directly to
the provider. That request deliberately implements neither serialization nor
debug/display formatting. Plans and advertisements contain only semantic limits,
resource requirements, authority requirements, and opaque identities; credential
bytes never enter plan identity, reports, attachment facts, evidence, or ordinary
diagnostics.

Successful execution yields a finite `NetworkAttachment` naming its exact host,
boot, resource pool, attachment identity, and generation. The attachment contains
no credentials, IP address, socket, carrier, or route. It says only that this boot
currently has an admitted network attachment. WebSocket initialization, Conduit
link observation, route candidates, physical Pico association, DHCP/DNS behavior,
durable secrets, discovery, and failover are later slices.
