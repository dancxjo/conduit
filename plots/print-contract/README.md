# Finite print contract

Check the ordinary three-request Plot without executing effects:

```sh
conduit check plots/print-contract/main.conduit --json
```

Its three input ports carry `PrintRequest` values with exact bounded document
resource identity/version, media, color, sides and copies. The selected Host
resolves the document and its authority independently. There is no portable
printer name, device path, endpoint or document URL.

`semantics/process/print.conduit` owns the native data. Requests admit only
against the selected Back's typed `PrintSupport` property and an exact current
resource binding: identity, version, content profile, access class, authority,
byte and item extents must agree. The fixture admits mono duplex and color jobs
on supporting providers, and refuses A3 where its supplied facts omit A3.

One `PrintAttempt` records evidence for one admitted request and attempt ID;
it holds no document bytes and does not queue, select, send or retry jobs.
Queued, dispatched, acknowledged and completed remain distinct. Unknown ACK,
provider loss after dispatch and cancellation after dispatch yield `uncertain`.
Any known provider job ID is retained for later reconciliation. A terminal
attempt cannot silently dispatch again. Definitive pre-dispatch refusal and
cancellation remain separate from uncertain delivery.

Document references admit at most 16 MiB and 1,000 declared items. Copies are
1–100; IDs are nonempty and at most 128 UTF-8 bytes. Support lists are finite,
and maximum request/lifecycle frames fit the admitted 8 KiB port bound.

This is checked Source/Plan plus typed protocol-simulator evidence for #5556.
No printer Back is installed by the catalog. Generic shared-pool execution,
queue fairness, durable duplicate/uncertain-delivery reconciliation and optional
physical CUPS/IPP acceptance remain #5557, #5558 and #5559; parent #5229 stays open.
