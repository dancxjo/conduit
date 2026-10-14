// Semantic hello evidence is independent of the optional human-facing label.
export function requireHostedHelloOutput(output) {
  const lines = output.split(/\r?\n/);
  if (!lines.includes('FACE-TEXT bytes=13 hex=48454c4c4f2c20574f524c442e') ||
      !lines.some(line => /^plan .+ complete$/.test(line)) ||
      !lines.some(line => /^host std-host-1 boot boot-.+ profile rust-std protocol 1$/.test(line))) {
    throw new Error('Retained executable did not complete the canonical hello plot');
  }
}
