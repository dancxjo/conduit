// Adapter-boundary fixture only: it supplies declared spans, never tokenizes.
export function syntaxProjectionFixture(spans) {
  const memory = new WebAssembly.Memory({ initial: 3, maximum: 3 });
  let outputLength = 0, calls = 0;
  const runtime = {
    memory,
    conduit_syntax_input_capacity: () => 8192,
    conduit_syntax_input_ptr: () => 0,
    conduit_syntax_output_ptr: () => 16_384,
    conduit_syntax_output_len: () => outputLength,
    conduit_syntax_project(length) {
      calls++;
      const output = new TextEncoder().encode(JSON.stringify({
        protocol: 'conduit.syntax-highlight-projection@1', source_bytes: length,
        kinds: ['whitespace', 'comment', 'keyword', 'name', 'identity', 'string', 'number', 'literal', 'operator', 'delimiter'],
        spans: spans ?? (length ? [[0, length, 3]] : []),
      }));
      new Uint8Array(memory.buffer, 16_384, output.length).set(output);
      outputLength = output.length;
      return 0;
    },
  };
  return { runtime, calls: () => calls };
}
