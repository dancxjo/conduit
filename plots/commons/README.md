# Commons places

Run `cargo xtask check plots run --deterministic --plot commons` to check and
execute the actual Source through an installed std Plan and kernel Play.

Commons owns three exact named places: `commons/hub`, `commons/workshop` and
`commons/creche`. The typed `CommonsPlaces` Fore carries their names and open
semantic role `commons/place`, two containment relations and two directed
reachability relations from Hub. These are authored paths inside this finite
domain; they do not grant permission or assert a live transport Line.

The checked contract fixes all endpoints, role/relation meanings and relation
cardinalities. Names are bounded to 32 UTF-8 bytes. The 8 KiB runtime boundary
holds the 2,324-byte canonical specimen; the oracle checks exact emitted values,
Source/checked/expanded/Plan/Play identities and unchanged allocation capacity.
Unknown/duplicate places, self or wrong-meaning relations, missing/excess
relations, unknown roles and oversized names refuse before Play. Malformed
Boolean runtime input refuses without emitting a graph.

The domain Types live in this Source, with no global Room enum or renderer
geometry. Current Body/Plot actions belong to #5610 and linear/cross-Mask
traversal to #5611. This proves the #5609 Source contract and deterministic std
execution; it does not complete parent #5359's lifecycle, UI or release gates.
Commons is excluded from Workspace discovery and the initial Body until those
product integrations are ready.
