# Conduit

Conduit connects typed parts into bodies. A body is an instance of a form.
The first implementation runs one finite form on one local host.

## Reboot 0

The first acceptance test is:

```bash
just hello
```

It parses one tiny `panel 0` file, constructs one finite body, runs it locally, prints `Hello, world!`, and exits successfully.
