const MAXIMUM_MESSAGE_SAMPLES = 64;
const MAXIMUM_CHANNELS = 2;

function halfQ15(sample) {
  return sample >= 0
    ? Math.floor((sample + 1) / 2)
    : Math.ceil((sample - 1) / 2);
}

function gainMessage(value) {
  const samples = value?.samples;
  if (!Array.isArray(samples) || samples.length === 0 ||
      samples.length > MAXIMUM_MESSAGE_SAMPLES ||
      samples.some((sample) => !Number.isInteger(sample) || sample < -32768 || sample > 32767)) {
    return { ok: false, code: "unsupported-profile" };
  }
  return { ok: true, value: { samples: samples.map(halfQ15) } };
}

class ConduitBoundedGainProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.port.onmessage = (event) => {
      const { id, operation, value } = event.data ?? {};
      const result = operation === "gain-q15-half"
        ? gainMessage(value)
        : { ok: false, code: "unsupported-operation" };
      this.port.postMessage({ id, ...result });
    };
  }

  process(inputs, outputs) {
    const input = inputs[0] ?? [];
    const output = outputs[0] ?? [];
    const channels = Math.min(input.length, output.length, MAXIMUM_CHANNELS);
    for (let channel = 0; channel < channels; channel += 1) {
      const source = input[channel];
      const target = output[channel];
      for (let frame = 0; frame < target.length; frame += 1) {
        target[frame] = (source?.[frame] ?? 0) * 0.5;
      }
    }
    return true;
  }
}

registerProcessor("conduit-bounded-gain", ConduitBoundedGainProcessor);
