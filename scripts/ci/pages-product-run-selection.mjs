export function selectExactSuccessfulRun(runs, headSha) {
  if (!Array.isArray(runs) || !/^[0-9a-f]{40}$/.test(headSha ?? "")) return undefined;
  return runs.find((candidate) =>
    candidate?.conclusion === "success"
    && candidate.head_sha === headSha);
}
