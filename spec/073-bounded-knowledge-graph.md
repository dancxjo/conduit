# Bounded cited-claim graph

`conduit-knowledge` owns entity and relation identities, cited claims, exact
graph snapshots, finite traversal queries, and source-retaining result paths.
Each entity and relation carries its own schema identity and schema version. A
claim is distinct from the source passage, citation, entity, relation, graph
snapshot, query, provider, and run evidence.

The deterministic first provider accepts one `knowledge/citation` produced by
the source-grounded retrieval boundary, assembles one fixed public claim, seals
it into one exact complete snapshot, and executes one finite traversal. The
exact plan pins the graph schema, snapshot, provider, entity schema, relation
schema, claim, confidence descriptor, validity interval, sensitivity, and all
depth, breadth, path, result, retained-byte, work, queue, and evidence limits.
Registering these contracts installs neither retrieval nor graph providers.
A host may install the retrieval provider while graph contracts remain
contract-only.

Every traversed edge must retain its own exact citation. A cited entity or an
adjacent cited edge never lends support to another claim. Generated assertions,
connectivity, confidence, and run evidence are not source support. Missing,
unsupported, contradicted, superseded, stale, partial, unauthorized, sensitive,
cancelled, and provider-lost outcomes remain distinct.

The first profile admits at most eight claims, depth two, breadth four, four
paths, four results, 4,096 retained bytes, 128 work units, and 64 evidence
events. The checked executable lesson deliberately requests a narrower
one-claim, one-hop, one-path, one-result traversal.

The graph contract is not a database API, ontology, implicit entity resolver,
truth probability, ambient store, or mandatory host service. A later provider
may use a graph database internally only by satisfying the same exact bounded
contracts and publishing its own implementation, artifact, resources, limits,
host observation, and evidence.
