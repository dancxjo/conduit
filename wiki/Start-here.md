You can meet Conduit at three levels: **look at it**, **run it hosted**, or **boot ConduitOS**.

## 1. Look before building

The body Workspace and visual journeys are the quickest entrances:

- [body Workspace](https://dancxjo.github.io/conduit/workspace/)
- [ConduitOS visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)
- [Current product truth](https://dancxjo.github.io/conduit/current-product.html)

The Workspace exposes resident forms. The ConduitOS journey is evidence from a real QEMU boot, not a mock screenshot deck.

## 2. Run the first hosted form

From a checkout of `dev`:

```bash
cargo xtask doctor
cargo xtask make host std
```

The second command checks, plans, and executes [forms/hello/main.conduit](https://github.com/dancxjo/conduit/blob/dev/forms/hello/main.conduit) through the production kernel.

```conduit
form hello {
    upper: text/upper
    show: presentation/text

    "Hello, world." >> upper >> show
}.
```

Read it left to right:

1. the literal produces text;
2. `text/upper` transforms it;
3. `presentation/text` presents it;
4. the trailing full stop says structural drain is semantic completion.

The form does not choose an OS-specific implementation. Planning does that later.

## 3. Run the integration truth loop

```bash
cargo xtask integrate
```

This exercises representative language, planning, kernel, hosted execution, body lifecycle, multi-placement, recovery, and Patchbay paths. It is deliberately broader than one example and narrower than release proof.

## 4. Open Patchbay

Native:

```bash
cargo xtask prove journey patchbay --on native
```

Browser:

```bash
rustup target add wasm32-unknown-unknown
cargo xtask prove journey patchbay --on browser
```

Patchbay is a projection over real body and execution truth. It is not another scheduler.

## 5. Boot ConduitOS

```bash
cargo xtask make conduitos live x86_64
cargo xtask make conduitos live-boot x86_64
```

The first builds the x86_64 live ISO. The second verifies and boots it in visible QEMU.

Inside the graphical system, birth a body, wake it, run resident forms, and inspect the same plan/play truth through Patchbay.

## 6. Read source, not just screenshots

A good beginner sequence is:

1. [Hello](https://github.com/dancxjo/conduit/blob/dev/forms/hello/main.conduit)
2. [Clock](https://github.com/dancxjo/conduit/blob/dev/forms/clock/main.conduit)
3. [Memory Lantern](https://github.com/dancxjo/conduit/blob/dev/forms/memory-lantern/main.conduit)
4. [Pocket Theremin](https://github.com/dancxjo/conduit/blob/dev/forms/pocket-theremin/main.conduit)
5. [Desk Telegraph](https://github.com/dancxjo/conduit/blob/dev/forms/desk-telegraph/main.conduit)
6. [Body Chat](https://github.com/dancxjo/conduit/blob/dev/forms/body-chat/main.conduit)

Then read [[Conduitese by example|Conduitese-by-example]].

## Proof has a scope

A successful hosted run proves the hosted path you ran. A QEMU run proves the specified emulated machine. Firmware compilation does not prove a physical board. A generated transcript does not prove a human heard speech.

That discipline is part of Conduit's design, not paperwork bolted on afterward. See [[Evidence and proof|Evidence-and-proof]].
