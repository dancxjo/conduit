Start with the [interactive Handbook](index.html). You do not need to install
Rust or run a server to use the published site. Your browser creates a local
Conduit body for your examples, or recovers the one it already keeps.

## 1. Predict what the clock will do

In the Handbook workbench, choose **A clock you can stop**. Read its source:
`time/every(1s)` produces ticks at an interval, and `>>` connects them to
`presentation/tick`.

What would happen if you changed `1s` to `2s`? Make a prediction before running
it.

## 2. Try it, then change it

Choose **Try in my Handbook**. Watch the ticks in the running example.

Change `1s` to `2s` in the source editor, then choose **Try in my Handbook**
again. Compare the pace. The editor highlights your source while you type;
checking and execution come from the Conduit runtime. If a change is refused,
read the refusal and correct the source before trying again.

## 3. Look inside your running example

Choose **Open in Patchbay**, then select the clock from the resident plot list.
The diagram comes from the actual plot in your body.

Follow the cord from the clock's output to the display's input. Select a gear,
port, or cord to inspect its facts. Port labels tell you what kind of information
can cross that connection. You can also use Tab and Enter to select subjects.

A **plot** describes the work. A **plan** selects how it can run. A **play** is
its execution. Expand **Inspect exact evidence** when you want to inspect their
identities; you do not need to memorize them to use the example.

## 4. Stop, return, and continue

Choose **Return to example** to see the clock again. **Lull** stops its current
execution; **Wake** starts another run.

Reload the Handbook. In the same browser storage, your local body and installed
examples are recovered. The browser starts a fresh boot and admits fresh
execution; it does not restore the old play as if it were still running.

**Your body and browser** shows storage status and offers **Start my Handbook
over**. Starting over forgets this application's body and examples while keeping
the browser host identity. Another browser has its own body. Clearing local
application storage can lose yours.

## Next: turn keystrokes into text

Choose **Turn keystrokes into text**, try it, and focus the running example
before typing. Open Patchbay to follow the keyboard input through the plot to
visible text. Memory Lantern does not promise to retain your typed message
across reload, even though the Handbook body itself has local continuity.

Continue with:

- [[Conduitese by example|Conduitese-by-example]] to read and write plots.
- [[Bodies, hosts, plans, and plays|Bodies-hosts-plans-and-plays]] to understand
  the identities you inspected.
- [[Glossary]] when a word is unfamiliar.
- [[Evidence and proof|Evidence-and-proof]] to distinguish browser execution,
  recorded demonstrations, emulation, and physical operation.

## Optional: run from a checkout

For repository work, follow the
[contributor setup](https://github.com/dancxjo/conduit/blob/dev/CONTRIBUTING.md),
then run the supported hosted entrance:

```bash
cargo xtask make host std
```

It checks, plans, and executes the Hello plot through the production kernel.
This is an optional development workflow; the published Handbook does not need
it to run your browser examples.
