# Hello, light

Type a message into the Form, press Run, and watch meaning become a timed indication. The browser light is one Host realization; the Form does not name a screen, an LED, a GPIO pin, or a board.

```conduit run
form hello-light {
    message: text/literal("SOS")
    morse: text/morse(120)
    light: presentation/indicator

    message > morse > light
}
```

The pattern between `morse` and `light` is bounded and portable: on or off, for a finite number of Morse units. Its reverse transform is canonical too, so a later chapter can take timed presses from a Pico or ESP32 button and turn the dots and dashes back into text without changing their meaning.
