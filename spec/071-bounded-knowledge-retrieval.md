# Bounded source-grounded retrieval

`conduit-knowledge` owns documents, exact source revisions and spans, index
snapshots, embedding identities, queries, retrieval results, and citations.
These are separate values. A citation never becomes true because generated text
mentions it, and execution evidence never substitutes for source evidence.

The first provider is a deterministic, finite fixture. Its exact plan pins the
implementation and artifact separately from the contract. It retains at most
1,024 bytes, accepts one 256-byte document and one 64-byte query, returns one
result, and performs at most 4,096 units of comparison work. Registering the
contracts does not install that provider or invoke a model, network, or store.

Missing, deleted, stale, partially indexed, incompatible-embedding, denied,
overflow, cancellation, and provider-loss outcomes remain distinct. Durable
stores, remote retrieval, generated answers, and graph claims are outside this
contract.
