# Native HTTP integration stack failure

The keyed image linked above its 36 KiB stack floor and passed preparation, but
its first publication hit the CPU stack guard. No HTTP reading was produced.
The retained full UART capture and ELF are outside Cargo output under the capture
listed in `failed-run.json`. The return path includes primitive quantity validation
and unit decoding. Comparing ELF disassembly showed the target publisher frame
had grown from 80 to 3,776 bytes because Mask preparation scratch remained live
across semantic publication. The correction separates those synchronous phases
with an explicit non-inlined function boundary; neither heap nor stack floor is
reduced. A link alone is insufficient evidence that this corrects live execution.
The protected operational image must be restored and fresh display updates verified
before ending a hardware experiment. Subsequent successful runs are separate proof.
