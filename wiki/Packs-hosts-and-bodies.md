## One Conduit language for plot, host, body and pack source

The repository has four canonical `.conduit` document roles:

~~~text
plot      portable semantic meaning
host      intended finite Host construction
body      intended Body/Part/Host construction and spore binding
pack      finite source/shipment/dependency description
~~~

They share one tokenizer, declaration/value syntax, spans and diagnostic model. There is no separate public TOML/YAML/JSON authoring language for host or body construction.

Canonical source families include:

~~~text
program.conduit
machine.host.conduit
house.body.conduit
~~~

Current `*.host.conduit` source lowers into the accepted checked host configuration/profile -> build -> image path.

Current `*.body.conduit` source composes checked host sources and bounded body/spore construction metadata.

Construction source never makes live runtime truth:

~~~text
host source
  != HostId / BootId
  != current offers / observations / authority
  != plan / play

body source
  != current membership/presence
  != current lines
  != plan / play
~~~

body birth itself consumes checked ordinary plots as the initial workload under the current body lifecycle; there is no privileged Seed semantic identity.

Do not invent another parser or embed TOML/YAML blobs inside `.conduit`.

The exact **existing** host/body grammar is implemented. New fields/declaration plots needed for richer mask/resource/facility composition must be earned as minimal general extensions, not treated as permission to redesign the document roles.

Provenance: #1752, #1780, #2284. Living native/host pressure test: #4117.


---

## Generics

Reusable plots use named type parameters such as `item: type`, with named
application arguments when inference is insufficient. Native type families
have a separate checked surface, `type Pair<T> = ...` and `Pair<Text>`. Both
specialize to finite checked meaning before play. See
[[Current language surface|Current-language-surface#generic-native-types]].

Plot signature sketch; the body is omitted, so `...` is not runnable source:

```conduit
plot latest (
    item: type

    >> values: item...
    current: $item >>
) {
    ...
}
```

**STATUS: FROZEN.** `name: type` is canonical; explicit application uses ordinary named arguments where inference is insufficient. See #4059.

No implicit `any`, wildcard zoo, or runtime type erasure requirement.

Provenance: #4059.

---

## Packs, imports, resolution and distribution

Keep these identities distinct:

~~~text
semantic Kind path
authored Plot/source identity
pack identity + pack version
module/source path
local alias
resolved pack content digest
distribution location
git repository + commit
installed artifact
Back implementation/artifact identity
~~~

A slash-separated kind path is not a pack path, module path, URL, git identity or installed artifact.

Canonical ecosystem laws:

- packs explicitly ship their public source surface;
- imports never grant runtime authority;
- build-time pack acquisition is separate from runtime network/line effects;
- pack version is not kind semantic contract identity;
- git commit is provenance/distribution truth, not semantic identity;
- accepted builds resolve a finite exact dependency graph;
- a lock representation pins exact content/versions/commits and contains no credentials or machine-specific absolute paths;
- locked local content permits offline check/build;
- different carriers may provide identical pack content;
- acquired bytes are untrusted until digest/schema/pack validation succeeds;
- no arbitrary install/build scripts by default.

Source import spelling is canonical `with` syntax:

~~~conduit
with audio/plots/tone
with math/geometry/{vector2, matrix2}
with house/sensors/temperature as room-temperature
~~~

`with` aliases may be ordinary identifiers or admissible gear glyphs under #4335. Pack authoring uses `pack.conduit`; generated exact lock truth uses `conduit.lock`.

Pack versioning may use familiar version syntax for ecosystem convenience, but Conduit's checker decides semantic compatibility; version numbers are not a universal compatibility oracle.

Provenance: #4054–#4058.

---
