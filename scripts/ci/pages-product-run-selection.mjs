export function selectExactSuccessfulRun(runs, headSha) {
  if (!Array.isArray(runs) || !/^[0-9a-f]{40}$/.test(headSha ?? "")) return undefined;
  return runs.find((candidate) =>
    candidate?.head_sha === headSha
    && candidate.status === "completed"
    && candidate.conclusion === "success");
}

export function selectExactRun(runs, headSha) {
  if (!Array.isArray(runs) || !/^[0-9a-f]{40}$/.test(headSha ?? "")) return undefined;
  return runs.find((candidate) => candidate?.head_sha === headSha);
}
