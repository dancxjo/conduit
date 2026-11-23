// Observe calls to the real Web Audio adapter while preserving playback.
export async function observeRealAudio(page) {
  await page.addInitScript(() => {
    globalThis.__cueAudio = { starts: [], ended: 0 };
    const start = AudioBufferSourceNode.prototype.start;
    AudioBufferSourceNode.prototype.start = function(...args) {
      globalThis.__cueAudio.starts.push({ frames: this.buffer.length, sampleRate: this.buffer.sampleRate, channels: this.buffer.numberOfChannels, state: this.context.state });
      this.addEventListener('ended', () => globalThis.__cueAudio.ended++, { once: true });
      return start.apply(this, args);
    };
  });
}
