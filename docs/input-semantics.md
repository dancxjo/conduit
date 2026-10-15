# Portable keyboard semantics

`input/keymap` revision `conduit.input/keymap@3` accepts an authored, typed
`KeymapLayout` value. Omitting `layout` selects US International with AltGr dead
keys. The mappings and composition exceptions are owned by
[`keymap-layouts.conduit`](../semantics/human/keymap-layouts.conduit); the portable
interpreter is [`PreparedKeymap`](../semantics/human/src/keymap_layout.rs).

## Author layouts in Conduitese

Local immutable values compose reusable layers. Later layers override earlier
bindings with exactly the same physical `usage`, `shift`, and `altgr` fields.
An explicit `none(empty)` binding suppresses that combination. Unbound
combinations produce no text; they do not fall back to another modifier level.

```conduit
plot memory_lantern_layered {
    base = [{ usage: 4, shift: false, altgr: false, action: text("a") }]
    overlay = [{ usage: 4, shift: false, altgr: false, action: text("λ") }]
    layout = { layers: [base, overlay], compositions: [] }
    keyboard: input/keyboard
    keymap: input/keymap(layout = layout)
    message: text/edit(maximum-bytes = 256)
    show: face/text("Layered message")

    keyboard.key >> keymap.key
    keymap.text >> message.fragment
    message.current >> show.text
}
```

This intentionally small layout maps physical A to `λ`. The complete runnable
example is [`layered.conduit`](../plots/memory-lantern/layered.conduit).
A layout is a value, so a reusable Plot can accept `layout: KeymapLayout` and
forward it to `input/keymap(layout = layout)`.

The source schema defines `KeymapBinding`, `KeymapLayer`, `KeymapLayout`, and
`KeymapComposition`. Each binding selects one action:

| Action | Meaning |
|---|---|
| `none(empty)` | Produce no text and preserve pending dead keys. |
| `text("λ")` | Emit the authored UTF-8 text. |
| `caps_text({ ordinary: "é", locked: "É" })` | Explicit text for CapsLock off/on, retaining this binding’s Shift level. |
| `caps_lock(empty)` | Toggle the interpreter’s CapsLock state. |
| `num_lock(empty)` | Toggle the interpreter’s NumLock state. |
| `keypad("1")` | Emit text when NumLock and Shift select numeric keypad mode. |
| `dead({ spacing: "´", combining: 769 })` | Retain a spacing scalar and a Unicode combining scalar. |

Layouts contain one to four layers, each with at most 128 bindings, and at most
64 composition exceptions. Text actions contain at most eight UTF-8 bytes; a
dead key's spacing form is one scalar occupying at most four bytes. Unicode
scalars exclude surrogates and values above U+10FFFF. Preparation validates the
finite value before interpreting events. Each exception names `combining`,
`base`, and `result` Unicode scalars; later matching exceptions take precedence.

## Saved language selection in Memory Lantern

[Memory Lantern](../plots/memory-lantern/main.conduit) authors a six-option
`input/saved-choice` with stable identities for US International, US, UK,
German, French, and Spanish. Its `chosen` current value connects directly to
`keymap.selection`. The keymap prepares the source-defined `choices` registry
before Play and switches among those exact typed layouts without rebuilding the
message editor. A new layout clears pending dead keys; selecting the same layout
preserves an unfinished composition.

The browser presents this primitive through the ordinary semantic named-select
renderer. It restores the preference before interpreting the first key and saves
each change automatically. “Saved in this browser” follows the confirmed storage
transaction; reopening restores that choice. Missing storage, a failed write,
a malformed saved record, and an unknown identity remain refusals. This is
browser-local persistence, subject to the browser's storage retention policy.

The selector's label, options, default, and preference identity are Conduitese
values. The browser adapter supplies presentation and durable storage. The
[keyboard-only companion](../plots/memory-lantern/keyboard-only.conduit) remains
available for Hosts without a saved-choice implementation.

## Default US International with AltGr dead keys

The reference is XKeyboardConfig's
[`us(altgr-intl)`](https://gitlab.freedesktop.org/xkeyboard-config/xkeyboard-config/-/blob/master/symbols/us),
which extends `us(intl)`. It keeps ordinary ASCII punctuation literal and places
dead keys on Right Alt. The checked Conduitese source holds all four levels:
plain, Shift, AltGr, and AltGr+Shift. Left and right Shift select the same level.
A third authored layer defines lock and numeric keypad behavior; the independent
plain US layout has its ASCII layer and the same keypad/lock layer. Its ISO extra
key follows XKB `pc105`: `<`, `>`, `|`, and `¦` across the four levels.

| Physical key | Plain | Shift | AltGr | AltGr+Shift |
|---|---|---|---|---|
| Grave | `` ` `` | `~` | dead grave | dead tilde |
| 1 | 1 | ! | ¹ | ¡ |
| 2 | 2 | @ | ² | dead double acute |
| 3 | 3 | # | ³ | dead macron |
| 4 | 4 | $ | ¤ | £ |
| 5 | 5 | % | € | dead cedilla |
| 6 | 6 | ^ | dead circumflex | ¼ |
| 7 | 7 | & | dead horn | ½ |
| 8 | 8 | * | dead ogonek | ¾ |
| 9 | 9 | ( | ‘ | dead breve |
| 0 | 0 | ) | ’ | dead ring above |
| Minus | - | _ | ¥ | dead dot below |
| Equal | = | + | × | ÷ |
| Q | q | Q | ä | Ä |
| W | w | W | å | Å |
| E | e | E | é | É |
| R | r | R | ë | Ë |
| T | t | T | þ | Þ |
| Y | y | Y | ü | Ü |
| U | u | U | ú | Ú |
| I | i | I | í | Í |
| O | o | O | ó | Ó |
| P | p | P | ö | Ö |
| Left bracket | [ | { | « | “ |
| Right bracket | ] | } | » | ” |
| A | a | A | á | Á |
| S | s | S | ß | § |
| D | d | D | ð | Ð |
| F | f | F | f | F |
| G | g | G | g | G |
| H | h | H | h | H |
| J | j | J | ï | Ï |
| K | k | K | œ | Œ |
| L | l | L | ø | Ø |
| Semicolon | ; | : | ¶ | ° |
| Apostrophe | ' | " | dead acute | dead diaeresis |
| Z | z | Z | æ | Æ |
| X | x | X | œ | Œ |
| C | c | C | © | ¢ |
| V | v | V | ® | ™ |
| B | b | B | · | dead stroke |
| N | n | N | ñ | Ñ |
| M | m | M | µ | ± |
| Comma | , | < | ç | Ç |
| Period | . | > | dead dot above | dead caron |
| Slash | / | ? | ¿ | dead hook above |
| Backslash | `\` | &#124; | ¬ | ¦ |
| ISO extra key | `\` | &#124; | `\` | &#124; |

Enter emits newline, Backspace emits U+0008, and Space emits a space on all four
levels. Tab emits no text. CapsLock and NumLock begin off. CapsLock reverses
Shift only for authored text pairs that differ solely by single-character case,
including AltGr accented letters; symbols and non-case pairs such as `ß`/`§`
stay on their authored level. Explicit `caps_text` actions handle native
exceptions: French unshifted `é`, `è`, `ç`, `à`, and `ù` become uppercase under
CapsLock while their shifted digits and `%` remain unchanged; German `ß`
becomes `ẞ` while Shift still emits `?`. NumLock selects keypad digits and decimal point;
Shift reverses that selection. When numeric mode is off, those keys emit no
text, leaving navigation to a separate semantic consumer. Keypad `/`, `*`, `-`,
`+`, Enter, and `=` emit their text regardless of NumLock or Shift. These
authored keypad bindings are available with and without AltGr. Lock key release
events do not toggle their state. Navigation, function keys, unmapped keys,
modifier-only events, and releases produce no text.

The seventeen dead-key families are grave, tilde, acute, diaeresis, circumflex,
horn, ogonek, double acute, macron, cedilla, breve, ring above, dot below, dot
above, caron, hook above, and stroke. Unicode canonical normalization supplies
composition; the authored exceptions cover stroke letters and symbols that
have no canonical composition, including `ø`, `ł`, `đ`, and `≠`.

Dead keys stack up to four distinct combining marks. For example,
AltGr+7, AltGr+Apostrophe, O produces `ớ`; changing the final key to Shift+O
produces `Ớ`. Marks are canonically ordered, including when entered in the
opposite order. Applying a mark to an already accented base also preserves its
canonical decomposition.

Space emits the pending spacing forms. Repeating a pending mark emits all
pending spacing forms and clears them. A fifth mark emits the four pending
spacing forms and starts a new prefix with the fifth. Escape cancels the prefix.
Backspace cancels a pending prefix without deleting editor text; otherwise it
emits U+0008 for the editor. An unsupported single dead-key combination emits
the spacing form followed by the original text. Stacked combinations retain
uncomposed marks in canonically ordered Unicode text. Each application emits
at most 48 UTF-8 bytes and retains no editor buffer.

Either Control, Left Alt, or either GUI modifier suppresses text and preserves
pending marks. Release and modifier-only events also preserve the prefix.
Closing or resetting the interpreter clears pending state. The default has no
GUI Compose or hexadecimal Unicode-entry mode. The explicit historical
`ConduitIntlKeymap` library retains its separate legacy behavior.

## Editor insertion point

Memory Lantern opts into `face/text("Remembered message", insertion-caret = true)`.
`face/text` semantic revision `conduit.std/face-text@2` includes this Boolean as a semantic declaration that insertion occurs at the end of the
current text. The editor emits its initial empty Current before any key input,
so the insertion point can be presented immediately. The bounded `text/edit` operation appends text and removes the last
scalar on Backspace; it does not implement arbitrary-position navigation or
selection. The cue does not add characters to the message or its saved value.
Browser manifestation carries `insertion_caret: true` separately from text.

Caret paint, blink, transitions, typography, and decoration belong to browser
Look/style realization. They do not grant editing actions or alter Face text.
This cue uses the existing configuration grammar; it introduces no new Look DSL.
The configuration defaults to false for ordinary text Faces. A presenter that
cannot realize this requested insertion cue must refuse it explicitly; native
serial-only text presentation accepts the default and refuses the opt-in.

The native graphical renderer does not yet implement this insertion cue or the
saved-choice control. The full Memory Lantern form therefore has no native
graphical parity claim; `keyboard-only.conduit` exercises its portable keyboard
behavior. Native layout and focus work is tracked in
[#5357](https://github.com/dancxjo/conduit/issues/5357), and shared visual Look
migration in [#5627](https://github.com/dancxjo/conduit/issues/5627). Browser CSS
is a host realization, not a stylesheet to execute in ConduitOS.

## Language presets

Six authored presets are available: US International with AltGr dead keys,
plain US, UK, German, French AZERTY, and Spanish. Their identities, labels, and
source definitions are declared in `input/keymap-presets` as typed Conduitese
descriptors. Build preparation resolves those descriptors into `KeymapChoices`: a
finite sequence of `{ identity, label, layout }` values, with each layout retained
as its actual typed value. The latter four follow the
installed XKeyboardConfig basic variants, including their inherited Latin
levels and native dead keys. Their complete four-level tables live alongside
the default in `keymap-layouts.conduit`; each has the shared authored lock and
keypad policy. German uses a comma for the numeric keypad decimal key.

| Preset | Physical-position examples |
|---|---|
| UK | Shift+2 → `"`, Shift+3 → `£`, Shift+Apostrophe → `@`. |
| German | Physical Y → `z`, Z → `y`, brackets/semicolon/apostrophe → `ü`/`ö`/`ä`. |
| French AZERTY | Physical Q/W/A/Z → `a`/`z`/`q`/`w`; unshifted 2 → `é`, Shift+2 → `2`. |
| Spanish | Semicolon → `ñ`, Backslash → `ç`, Equal → `¡`, Shift+Equal → `¿`. |

The source references are XKeyboardConfig's
[UK](https://gitlab.freedesktop.org/xkeyboard-config/xkeyboard-config/-/blob/master/symbols/gb),
[German](https://gitlab.freedesktop.org/xkeyboard-config/xkeyboard-config/-/blob/master/symbols/de),
[French](https://gitlab.freedesktop.org/xkeyboard-config/xkeyboard-config/-/blob/master/symbols/fr),
and [Spanish](https://gitlab.freedesktop.org/xkeyboard-config/xkeyboard-config/-/blob/master/symbols/es)
`basic` variants. Their additional dead keys use the same generic composition
engine; German also exposes a macron-below dead key.

## Platform event seams

Platforms supply physical key usages and exact left/right modifier after-state
through `input/key-event@1`. They do not supply localized text to this map.
Browser keyboard codes, USB usages, toolkit events, and operating-system locale
remain adapter facts. Browser AltGraph handling normalizes its platform modifier
representation at that seam; the authored layout uses the same Right Alt bit
on every target. Unknown codes, duplicate presses, unmatched releases, focus
loss, queue pressure, cancellation, and closure remain distinct.

## Chords and typed fan-out

`input/chord@1` contains a modifier snapshot, portable key usage, `Triggered`
phase, and canonical chord ID. `input/chords` uses the `conduit-core` map:

| Combination | Canonical ID |
|---|---|
| Ctrl+G | `chord/cancel-or-escape` |
| Ctrl+L | `chord/clear-or-refresh` |
| Ctrl+R | `chord/repeat-or-replan` |
| LeftAlt+P | `chord/palette` |
| LeftAlt+I | `chord/inspect` |
| LeftMeta+P | `chord/plan` |
| LeftMeta+Space | `chord/command` |
| LeftMeta+Enter | `chord/activate` |

Left/right modifier identity remains encoded. Right Alt and Right Meta are not
chord namespaces in this map. Chord IDs are semantic hints; the interpreter
executes no product action.

`input/key-tee` delivers each key value atomically to `text-keys` and
`chord-keys`, or waits under pressure. It does not implicitly drop or broadcast
values. The keymap and chord operations each admit at most sixteen input actions
and one in-flight Host Call per gear.

Native Observatory JSON exports retain the complete typed Offer and Plan under a
2 MiB encoded-output bound; larger snapshots refuse export explicitly. This
preparation/export retention budget is separate from connection storage and uses
the boot arena. Published native product and ordinary keyboard proof profiles
explicitly admit a 32 MiB preparation heap, independently of their 8 MiB static
budget. After eliminating whole-tree copies, the IA32 emulator product measured
18,278,752 bytes at peak and 117,440 bytes live after preparation under both UEFI
and legacy BIOS; 8 MiB and 16 MiB diagnostic heaps exhausted their finite arenas.
These are emulator measurements, not physical-device proof or a guarantee that
all maximum-size authored configurations fit. The six-layout keyboard proof
snapshot is approximately 1.3 MB; canonical layout values remain
bounded by the shared 256 KiB Structured carrier limit and the authored registry
admits at most eight choices.
