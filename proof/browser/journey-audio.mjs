// Offline voicing of exact retained Speech segments, not Conduit runtime playback.
import { readFile, writeFile, realpath, stat, copyFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { dirname, resolve, sep } from "node:path";
import { execFileSync } from "node:child_process";

const [trackPath, piper, model, config] = process.argv.slice(2);
if (!trackPath || !piper || !model || !config) throw new Error("expected track, Piper, model and config");
const root = await realpath(dirname(trackPath));
const sha = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const bounded = async (path, maximum = 1024 * 1024) => {
  if ((await stat(path)).size > maximum) throw new Error(`oversized documentary input: ${path}`);
  return readFile(path);
};
const file = async relative => {
  const path = await realpath(resolve(root, relative));
  if (!path.startsWith(`${root}${sep}`)) throw new Error("documentary input escaped track");
  return path;
};
const track = JSON.parse(await bounded(trackPath));
if (track.track_id !== "hosted-generative" || track.steps.length !== 13) throw new Error("expected bounded generative track");
const run = (command, args, input) => execFileSync(command, args, { input, timeout: 60_000, maxBuffer: 2 * 1024 * 1024 });
const tool = {
  schema: "conduit.evidence/documentary-voicing@1",
  boundary: "Offline Piper voicing of exact outward model words. Not runtime speech/synthesize or speaker playback proof.",
  piper_sha256: sha(await bounded(piper, 128 * 1024 * 1024)),
  model_sha256: sha(await bounded(model, 256 * 1024 * 1024)),
  config_sha256: sha(await bounded(config)),
  ffmpeg_version: run("ffmpeg", ["-version"]).toString().split("\n")[0],
};
const cache = new Map();
for (const step of track.steps) {
  const transcript = step.evidence.find(item => item.evidence_class === "transcript");
  if (!transcript) continue;
  if (!/^[a-z.-]+$/.test(step.step_id)) throw new Error("invalid step id");
  if (step.evidence.some(item => item.evidence_class === "audio")) throw new Error("audio already retained");
  const text = await bounded(await file(transcript.path), 16_384);
  if (sha(text) !== transcript.sha256) throw new Error("transcript digest changed");
  const evidence = step.evidence.find(item => item.evidence_class === "presenter-receipt");
  if (!evidence) throw new Error("transcript lacks original Presenter receipt");
  const original = await bounded(await file(evidence.path));
  if (sha(original) !== evidence.sha256) throw new Error("Presenter receipt digest changed");
  const proof = JSON.parse(original);
  const actual = proof.execution.manifestation.content.filter(segment => segment.role === "Speech")
    .map(segment => Buffer.from(segment.bytes).toString("utf8")).join("\n\n");
  if (actual !== text.toString("utf8")) throw new Error("transcript differs from model's outward speech");
  const stem = `artifacts/${step.step_id}-voice`;
  const wav = resolve(root, `${stem}.wav`);
  const mp3 = resolve(root, `${stem}.mp3`);
  const wave = resolve(root, `${stem}.png`);
  const cached = cache.get(transcript.sha256);
  if (cached) {
    for (const [from, to] of [[cached.wav, wav], [cached.mp3, mp3], [cached.wave, wave]]) await copyFile(from, to, 1);
  } else {
    // Refuse overwrite even though Piper itself permits it.
    await writeFile(wav, new Uint8Array(), { flag: "wx" });
    run(piper, ["--model", model, "--config", config, "--output_file", wav], text);
    const duration = Number(run("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1", wav]));
    if (!(duration > 0 && duration <= 120)) throw new Error("voice exceeds 120-second documentary bound");
    run("ffmpeg", ["-v", "error", "-n", "-i", wav, "-codec:a", "libmp3lame", "-b:a", "96k", mp3]);
    run("ffmpeg", ["-v", "error", "-n", "-i", wav, "-filter_complex", "showwavespic=s=960x160:colors=0xb8efc8", "-frames:v", "1", wave]);
    cache.set(transcript.sha256, { wav, mp3, wave });
  }
  const receipt = Buffer.from(JSON.stringify({ ...tool, transcript_sha256: transcript.sha256,
    manifestation_identity: proof.execution.manifestation.manifestation_identity }, null, 2));
  await writeFile(resolve(root, `${stem}.json`), receipt, { flag: "wx" });
  for (const [extension, kind, caption] of [
    ["png", "waveform", "Waveform measured from this voice recording."],
    ["mp3", "audio", "Hear the model's exact words, voiced by Piper for this documentary."],
    ["wav", "audio-source", "Lossless source recording of the documentary voice."],
    ["json", "audio-receipt", tool.boundary],
  ]) {
    const path = `${stem}.${extension}`;
    const bytes = await bounded(resolve(root, path), 16 * 1024 * 1024);
    step.evidence.push({ artifact_id: `${track.track_id}/${step.step_id}/${kind}`,
      evidence_class: kind, assertion_rung: "deterministic-observation",
      documentary_description: caption, path, sha256: sha(bytes) });
  }
  console.log(`Retained voice: ${step.step_id}`);
}
await writeFile(trackPath, `${JSON.stringify(track, null, 2)}\n`);
