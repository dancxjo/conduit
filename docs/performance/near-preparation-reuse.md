# Immutable Near preparation fragments

Each Std Host owns at most four FIFO Near strategy fragments. A fragment holds
only immutable initialized offer metadata and its exact key. It never holds a
mutable Back, output buffer, numerical scratch, reservation, grant or active
Play. Factory temporaries retire on the cold path, and dropping the Host releases
its cache references. Borrowed immutable owners survive eviction and retire when
their final reference drops.

The key compares Source/checked/expanded identities, the complete exact Gear
(including configuration, Type and law contracts), and protocol/Host/Boot/profile.
Exact Gear contents still distinguish changed requests with equal identity
strings. The installed Back and artifact implementation cannot change within
this process; this cache is neither serialized nor reused across binaries.
Errors are not retained and cannot evict a valid fragment. Entry bounds retain
caller-admitted finite Source/Type structures; they do not claim a byte-accurate
memory reservation.

Near fragments have no authority requirements. Current expansion validation,
active-reservation fencing, transactional advertisement publication, offer
generation, planning and execution remain outside the cache. Changes to resources,
authority or offer generation do not reuse decisions about those facts. A failed
batch may retain an independently successful immutable fragment, but publishes
no partial Host advertisement. All Host constructors begin with an empty owner.

The [local observation](../../proof/performance/numeric/near-preparation-reuse-20261020.json)
uses the ordinary numerical preparation method. Across 32 repeated calls,
allocation requests fall from 6,099,168 to 968,320 while the complete advertisement
and generation remain equal. The first cold call adds 110 requests for exact keys
and immutable ownership. The separate complete candidate Host-owner lifetime
(creation, 33 preparations and final destruction) makes 1,162,751 requests.
There is no matched full-owner reference measurement, byte/RSS/peak-memory
measurement, or calibrated product latency claim.

Local conformance compares complete cached/cold offer values; changed Source,
Type, law, precision and Host compatibility facts; malformed requests; FIFO and
borrowed-owner lifetime. The ordinary Std numerical corpus additionally compares
exact Plans, full kernel reports, output records and refusals.
