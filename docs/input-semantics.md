# Portable keyboard semantics

This document records the exact first portable text and modifier-chord maps.
Device, USB, DOM, toolkit, operating-system layout, locale, and product-action
facts are deliberately absent. The executable source of truth is the finite
allocator-free state in `conduit-core`.

## `input/keymap` and `conduit-intl`

`conduit-intl` is the only accepted layout in revision
`conduit.input/keymap@1` and is the default when `layout` is omitted. Ordinary
letter, digit, whitespace, and punctuation keys use familiar US-QWERTY base
and Shift layers. Enter emits newline. Tab, navigation, editing, function,
modifier-only, and release transitions emit no text.

Right Alt is AltGr. Its reviewed direct layer is:

| Keys | Text | Keys | Text |
|---|---|---|---|
| AltGr+A | `æ` | AltGr+O | `ø` |
| AltGr+S | `ß` | AltGr+D | `ð` |
| AltGr+P | `þ` | AltGr+N | `ñ` |
| AltGr+E | `€` | AltGr+L | `£` |
| AltGr+Y | `¥` | AltGr+C | `©` |
| AltGr+R | `®` | AltGr+T | `™` |
| AltGr+Shift+1 | `¡` | AltGr+Shift+/ | `¿` |
| AltGr+- | `–` | AltGr+Shift+- | `—` |

A tap of Right Meta starts Compose. `AltGr+Space` is the fallback prefix on a
keyboard without Right Meta. The first vocabulary is `' e → é`, `` ` e → è ``,
`^ e → ê`, `" e → ë`, `~ n → ñ`, `, c → ç`, `o a → å`, and `/ o → ø`.
Shifting the second letter produces the uppercase scalar. An unknown second
key refuses the sequence and resets; Escape cancels and resets.

Holding Right Meta and pressing U starts hexadecimal Unicode scalar entry.
`AltGr+Shift+Space` is its fallback prefix. One to six hexadecimal digits and
Enter emit one valid scalar. Empty input, a seventh digit, values above
U+10FFFF, surrogates U+D800–U+DFFF, and non-hex input refuse and reset. Escape
cancels. Each successful action emits one UTF-8 fragment of at most four
bytes; no line or editor buffer is retained.

Either Control, Left Alt, or Left Meta suppresses ordinary text. In particular,
Control combinations do not become C0 bytes, signals, cancellation, or desktop
actions.

## `input/chords` and `conduit-core`

`input/chord@1` is the exact four-byte structural value: modifier snapshot,
portable key usage, `Triggered` phase, and canonical chord ID. Revision
`conduit.input/chords@1` emits only registered pressed combinations; releases
and unknown combinations emit nothing. The default `conduit-core` table is:

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

Left and right modifier identity remains encoded. Right Alt and Right Meta are
never chord namespaces in this map because `conduit-intl` reserves them.
Chord IDs are semantic hints only; `input/chords` executes no action.

`input/key-tee` is the exact typed composition seam used when both meanings are
wanted. Each three-byte `input/key-event@1` value is admitted atomically to its
`text-keys` and `chord-keys` branches or waits under pressure. It does not copy,
drop, retry, reinterpret, or broadcast implicitly.

All three Kinds admit at most eight queued values. A std semantic operation
admits at most sixteen input actions and one in-flight host operation per
keymap/chord Gear. Compose retains at most one prefix scalar; Unicode entry
retains one scalar accumulator and a six-digit count. Cancellation clears the
single pending operation, and closing input closes the finite Gear.
