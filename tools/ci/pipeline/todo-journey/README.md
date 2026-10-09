# Todo journey capture and publication contract

The Todo page is deliberately absent until one live producer run supplies a
complete `site/evidence/todo-journey/` directory. A page build sees its
`manifest.json`, validates the complete bundle, copies it through a private
staging directory, verifies the copy, and only then makes
`journeys/current/todo/` visible. The Journeys catalogue adds the card only
when that page exists. Clock recordings keep their own routes and history.

This is a user journey, in this order: `birth`, `add`, `join`, `complete`,
`inspect`, `hear`, `read`, `recover`. Each chapter says what a person wants,
does, sees or hears, why that matters, and what to try next. Captures sit next
to those explanations; internal identities and limitations remain available
through disclosure. The page uses the shared site navigation, type, colours,
and responsive shell.

The evidence directory contains a complete `conduit.evidence-manifest/v1`
with `proof_id: journey-todo-one-body`, `suite_id: journey-gallery`, an exact
capture `git_commit` ancestral to the publication source, and at most 128
required outputs. Every output has a unique ID and safe relative path, one
`scenario_id` equal to the journey run, exact bytes and SHA-256, kind and media
type. Files not declared in the manifest, symlinks, files over 16 MiB, and
bundles over 128 MiB are refused. The output named `journey` is
`conduit.journey/todo@1` and carries `source_commit`, `run_id`, `body_id`, and
the eight ordered chapters. Every chapter has a receipt output, at least one
media output with useful alt text and capture receipt, and an explicit limit.
The journey names a `conduit.todo-journey/producer-terminal@1` output whose
completed `cargo xtask prove todo-journey` receipt enumerates every ordered
event and media output. The checked-in fixture can exercise this shape, but
cannot establish that the receipt came from a real command invocation.

Chapter receipts use `conduit.todo-journey/chapter-receipt@1`. They name the
same source/run/Body, chapter, unique event, increasing observation time,
resulting Face ID/revision, Show ID, and a declared producer event receipt.
`add` and `complete` also name the exact typed Face interaction and action.
They name distinct Mask kinds. `read` instead names the direct spoken Mask's
`read-current-items` ReaderCommand and exact Mask Play; it is a disclosure of
the current Face, not an invented Body mutation or Face action. Mutations require a queue
sequence, produced outcome, and correlated
child Sign; queue acceptance alone cannot describe the changed list. The
`conduit.todo-journey/producer-event@1` output repeats those identities and
attests each capture's output ID, source, and digest. Capture receipts use
`conduit.todo-journey/capture-receipt@1` and match their chapter event, Face,
Show, output, and producer receipt. Recovery names distinct old/new Boots and
an exact pre-lull state digest equal to the recovered state digest under the
same Body.

A complete run needs the three required Masks: a terminal capture, a graphical
screenshot from Chromium, QMP **or** a native provider, and direct speech in both
the status and requested-detail chapters. Additional graphical providers are
optional; one journey does not need to demonstrate all graphical Hosts.
A screenshot must be an actual PNG from its named source; a terminal
capture is a UTF-8 transcript. WAV captures record the **same delivered
Play**: the capture and producer receipts agree on Play, Plan, Show, PCM
format and delivered PCM digest. A selected speaker Play needs positive
committed frames. The WAV header and samples must match those claims. Guest
speech additionally needs the QEMU Boot, positive captured QEMU frames, and
the declared digest-verified QEMU audio output. Its captured PCM must be
byte-identical to the PCM inside the published WAV; packaging raw QEMU PCM in
a playable WAV may change the file hash without changing delivered samples.
Synthesis from
the same words in a different Play is not admissible. Model-assisted speech
may be included, but direct speech is required for the screen-free status and
requested-detail steps. A declared transcript must match the spoken-text
digest in both source and capture receipts, and its words appear alongside
the player. Automated playback is not attended human listening.

The current selected voice Boot reserves one create-new WAV destination. Two
direct recordings may require different Boots; each chapter must retain its
own exact Boot, Face, Show, Mask Play, speaker Play, and delivered PCM evidence.
That intermediate Boot does not substitute for the later recovery chapter.

The fresh first-chapter entrance is `cargo xtask prove todo-journey` with a
checked Todo Plot, an unowned installed Host, an empty selected checkpoint
directory, an exact-source static Handbook package, and pinned Playwright. It
births one Body and uses the real browser Mask to add the first item. It retains
the Birth output, before/after Owner Faces, browser receipt and screenshots,
and checkpoint inventories. For example:

```sh
cargo xtask prove todo-journey \
  --state-dir /path/to/new-installed-host-state \
  --conduit-bin /path/to/new-installed-host-state/releases/HASH/conduit-linux-x86_64 \
  --output /path/to/new-birth-add-capture \
  --fresh-body-source plots/todo/checkpoint-once.conduit \
  --handbook-package /path/to/exact-source-static-handbook \
  --pinned-playwright /path/to/pinned/playwright/index.mjs \
  --first-item-text 'Buy milk'
```

Its `partial-run.json` records `chapter_scope: ["birth", "add"]` and
`publication_ready: false`. The producer stops its foreground Owner service
after capture; later chapters must reopen that same installed Body. The
terminal encounter entrance can then read the current Body status and Face,
run an actual terminal Mask session, and read the status and Face again. For
example:

```sh
cargo xtask prove todo-journey \
  --state-dir /path/to/installed-host-state \
  --conduit-bin /path/to/installed-host-state/releases/HASH/conduit-linux-x86_64 \
  --output /path/to/new-capture-directory \
  --terminal-script /path/to/utf8-commands-ending-in-quit
```

The terminal `partial-run.json` records the exact installed release, command
outputs, digests, same-Body before/after Face revisions, and any refusal. The
terminal script is retained with the output, so use only journey content. The
default script reads the Face and quits; a supplied script may exercise actual
Face controls. Neither partial entrance claims to complete the Todo journey,
write `manifest.json`, or unlock publication. The remaining producer must
be a trusted
`cargo xtask prove` capture command that retains its own completed terminal
receipt and records the exact source commit, run, Body, Host/Boot, Plan/Play,
Face/Show, interaction, and media identities used in the per-chapter producer
receipts. A human reviewer or admission job must verify that terminal receipt
came from the command's actual run. This renderer checks only documentary
consistency; self-authored JSON and hashes cannot prove live execution.
The installed Owner can activate a Todo write, read its committed checkpoint
through an admitted Host Call in the same Body, then admit a second selected
action and verified read on that Body and Boot. Focused Owner tests establish
that sequence, including refusal of a stale Show. They do not yet establish
recovery after a fresh Boot or a live journey through browser, native,
terminal, and spoken Masks. The producer should
retain one Body ID, take each screenshot from the actual browser, native
display, and QMP guest in the same run, and capture speaker or QEMU output
from the exact selected speech Play. It must derive event and media receipts
from actual Owner/Body/Mask observations, not copy or edit old clock evidence.
The renderer verifies documentary consistency; only the producer and target
acceptance prove that real actions and audio delivery happened.

Candidate preflight automatically discovers the fixture tests in `proof/ci/pipeline-todo-journey.spec.mjs`. Their generated
images and WAV are rejection/layout fixtures only and are never staged for
publication. Run them with `node --test proof/ci/pipeline-todo-journey.spec.mjs` as an
internal development check; the supported site publication entrance remains
`cargo xtask` through the site pipeline.
