# Say it, then get it back

Morse is one meaning with two exact directions. Edit the quoted message, press Run, and the same ordinary Form encodes it into canonical timed Morse before decoding it back to text.

```conduit run
form morse-round-trip {
    message: text/literal("HELLO 2")
    encode: text/morse(unit-ms = 80)
    decode: morse/text
    result: presentation/text

    message > encode > decode > result
}
```

The browser editor admits your source through Conduit's bounded typed text-interaction contract before parsing it. Its evidence retains the exact proposal and result identities plus the byte count, but not a second copy of what you typed.
