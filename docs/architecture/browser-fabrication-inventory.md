# Browser Host fabrication inventory

The authoritative configurable inventory is `BROWSER_IMPLEMENTATIONS` in the browser Host fabrication package. Crèche, BUILD validation, and other configurators consume that package contribution; product code must not maintain another browser implementation list.

Every exposed entry is versioned, targets `browser/wasm32/page`, binds to `conduit.browser/reviewed-distribution@1` / `browser-runtime-superset.wasm`, and carries finite instance and buffered-byte limits. The shared artifact may contain all implementations, but PROFILE admission and current runtime truth remain separate gates.

| Runtime mechanism | Fabrication classification | Runtime prerequisite truth |
| --- | --- | --- |
| DOM presentation | selectable structural and portable presentation Bases | initialized surface |
| keyboard and pointer | selectable portable human-input Bases | focus/page lifecycle; no permission claim at BUILD |
| IndexedDB application storage | selectable durable-storage Base | secure context, availability, quota, schema and corruption checks |
| WebSocket | selectable Line Base | secure context plus endpoint and credential truth |
| WebRTC DataChannel | selectable Line Base | secure context plus negotiated session/grant truth |
| camera and microphone | selectable media Bases | secure context, user activation, permission, device acquisition |
| Web Audio output | selectable audio Base | user activation and current audio context |
| WebSerial and WebUSB | selectable device Bases | secure context, user activation, permission, explicit device acquisition |
| browser Host identity and Body membership | Host mechanism, not a configurable semantic Base | durable profile and admitted membership authority |
| application package loader/presentation bridge | Host operation and product substrate | exact admitted package bytes |
| Book runners and browser proof fixtures | application/proof code, never ordinary Host choices | intentionally excluded |
| touch and gamepad | intentionally unsupported | no reviewed production contract/implementation yet |

Implementation availability, PROFILE selection, Boot initialization, current resource/permission truth, and immutable Plan selection are distinct. BUILD records only the first two. In particular, selecting camera, microphone, WebSerial, or WebUSB never prompts for permission and never claims a device exists.
