export function selectExactSuccessfulRun(runs, headSha, pullNumber) {
  if (!Array.isArray(runs) || !/^[0-9a-f]{40}$/.test(headSha ?? "")) return undefined;
  return runs.find((candidate) =>
    candidate?.head_sha === headSha
    && belongsToPull(candidate, pullNumber)
    && candidate.status === "completed"
    && candidate.conclusion === "success");
}

export function selectExactRun(runs, headSha, pullNumber) {
  if (!Array.isArray(runs) || !/^[0-9a-f]{40}$/.test(headSha ?? "")) return undefined;
  return runs.find((candidate) => candidate?.head_sha === headSha && belongsToPull(candidate, pullNumber));
}

export async function resolveMergedPullSource(pull, loadCommit) {
  if (!pull?.merged_at || !/^[0-9a-f]{40}$/.test(pull.merge_commit_sha ?? "")
    || !/^[0-9a-f]{40}$/.test(pull.head?.sha ?? "") || typeof loadCommit !== "function") {
    throw new Error("pull request is not an exact merged source");
  }
  const mergedCommit = await loadCommit(pull.merge_commit_sha);
  if (!/^[0-9a-f]{40}$/.test(mergedCommit?.tree?.sha ?? "")) {
    throw new Error("pull request has no exact merged tree");
  }
  return {
    mergeCommit: pull.merge_commit_sha,
    sourceHead: pull.head.sha,
    sourceTree: mergedCommit.tree.sha,
  };
}

function belongsToPull(candidate, pullNumber) {
  if (pullNumber === undefined) return true;
  if (!Number.isSafeInteger(pullNumber) || pullNumber <= 0) return false;
  if (!Array.isArray(candidate?.pull_requests)) return false;
  return candidate.pull_requests.length === 0
    || candidate.pull_requests.some((pull) => pull?.number === pullNumber);
}
