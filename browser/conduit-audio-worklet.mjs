class ConduitPassThroughProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.port.onmessage = (event) => {
      const { id, operation, value } = event.data ?? {};
      this.port.postMessage(operation === "echo"
        ? { id, ok: true, value }
        : { id, ok: false, code: "unsupported-operation" });
    };
  }

  process(inputs, outputs) {
    const input = inputs[0] ?? [];
    const output = outputs[0] ?? [];
    const channels = Math.min(input.length, output.length);
    for (let channel = 0; channel < channels; channel += 1) {
      output[channel].set(input[channel]);
    }
    return true;
  }
}

registerProcessor("conduit-pass-through", ConduitPassThroughProcessor);
