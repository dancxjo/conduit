# Documentation ownership and proof language

This guide governs how Conduit describes itself. It does not supersede
[CONTRIBUTING](../CONTRIBUTING.md), [AGENTS](../AGENTS.md), the
[canon](conduit-canon.md), or any exact target acceptance test.

## One question, one primary owner

| Question | Primary owner | Linked explanations |
|---|---|---|
| Why does Conduit exist? | [README](../README.md) and [Why Conduit](../wiki/Why-Conduit.md) | Handbook tour |
| What must its architecture preserve? | [Canon](conduit-canon.md) | [Architecture reference](architecture/README.md) |
| What syntax is accepted on `dev`? | [Checked language reference](../wiki/Current-language-surface.md) plus source/tests | Tutorials and examples |
| What actually works, and with which limits? | [STATUS](../STATUS.md) plus named proof receipts | Target-specific instructions |
| What comes next? | Owning issue and current open PR | [Roadmap snapshot](roadmap.md) |
| What did an earlier run establish? | Immutable evidence and [history](history/README.md) | [Audits](audits/README.md) |
| How does someone run or contribute? | [Try Conduit](try-conduit.md) and [CONTRIBUTING](../CONTRIBUTING.md) | Target guides |

Do not copy a large status or language inventory into an introduction. Link
the owner and explain only enough to orient the current reader.

## Make claims with the right evidence

- **Implemented in source** means the relevant code and declared path exist.
- **Checked or compiled** is not an executed Plan/Play.
- **Deterministic, browser, emulator, physical hardware, and human enactment**
  are different proof classes. Name the one that actually occurred.
- **Developed on a draft branch** does not mean landed on `dev`; landed on
  `dev` does not mean accepted, released, or deployed.
- A downloaded image, built firmware, pretty preview, or mock provider does not
  establish physical execution of its proposed device.
- A finite, exact Plan is not a promise that every future resource loss can be
  recovered without explicit authority or new planning.
- A readiness observation is not a lifecycle transition or proof of retirement.

For changing facts, state the branch, commit or named checkpoint, and date where
appropriate. Prefer a link to the live issue or
[current product truth](https://dancxjo.github.io/conduit/current-product.html)
over manually copying volatile results.

## Three editorial layers

1. **Invitation:** concrete user benefit and first runnable experience;
   avoid lengthy ontology and release machinery.
2. **Explanation and reference:** define each term once, offer worked examples,
   link to the checker or owning contract, and distinguish design from evidence.
3. **Evidence and history:** preserve negative outcomes, refusals, measurements,
   raw receipt hashes, exact machines, and limitations without upgrading them
   into today's or tomorrow's general claim.

Retained historical records may intentionally use superseded syntax. Do not
rewrite their quotations or measurements simply to make them look current.
Instead add a contextual index pointing to the new owner. For active guides,
remove retired commands and describe the currently accepted spelling.

## The reader's route

A new reader should go from [README](../README.md) to
[Why Conduit](../wiki/Why-Conduit.md), then the
[architecture tour](../wiki/Architecture-tour.md) or a real
[Try Conduit](try-conduit.md) exercise. A contributor can proceed from
[CONTRIBUTING](../CONTRIBUTING.md) to the owning contract and tests.

Relative repository links should target actual files or existing directories.
Prefer canonical `wiki/` handbook Markdown over GitHub wiki duplicates. An
audit must report its scope honestly: counting all documents is not the same
as reviewing every claim or rerunning every proof.
