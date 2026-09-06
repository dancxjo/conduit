const PRODUCT_CARRIER_WORKFLOW_PATHS = new Set([
  ".github/workflows/promotion.yml",
  ".github/workflows/candidate.yml",
  ".github/workflows/tour-products.yml",
]);

export function productCarrierRuns(runs) {
  if (!Array.isArray(runs)) return [];
  return runs.filter((run) => PRODUCT_CARRIER_WORKFLOW_PATHS.has(run?.path));
}

export function hasRetainedArtifact(artifacts, name) {
  return Array.isArray(artifacts)
    && typeof name === "string"
    && name.length > 0
    && artifacts.some((artifact) => artifact?.name === name
      && artifact.expired === false
      && Number.isSafeInteger(artifact.size_in_bytes)
      && artifact.size_in_bytes > 0);
}

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
  const [mergedCommit, sourceCommit] = await Promise.all([
    loadCommit(pull.merge_commit_sha),
    loadCommit(pull.head.sha),
  ]);
  if (!/^[0-9a-f]{40}$/.test(mergedCommit?.tree?.sha ?? "")
    || !/^[0-9a-f]{40}$/.test(sourceCommit?.tree?.sha ?? "")
    || !/^[0-9a-f]{40}$/.test(mergedCommit?.parents?.[0]?.sha ?? "")) {
    throw new Error("pull request has no exact source, integration base, and merged trees");
  }
  return {
    mergeCommit: pull.merge_commit_sha,
    sourceHead: pull.head.sha,
    sourceTree: sourceCommit.tree.sha,
    integrationBase: mergedCommit.parents[0].sha,
    integrationTree: mergedCommit.tree.sha,
  };
}

export async function resolveExactMainSource(requestedSha, currentSha, loadCommit) {
  if (!/^[0-9a-f]{40}$/.test(requestedSha ?? "")
    || requestedSha !== currentSha || typeof loadCommit !== "function") {
    throw new Error("requested main commit is not the exact current branch tip");
  }
  const commit = await loadCommit(requestedSha);
  if (!/^[0-9a-f]{40}$/.test(commit?.tree?.sha ?? "")
    || !/^[0-9a-f]{40}$/.test(commit?.parents?.[0]?.sha ?? "")) {
    throw new Error("current main has no exact tree and integration base");
  }
  return {
    mergeCommit: requestedSha,
    sourceHead: requestedSha,
    sourceTree: commit.tree.sha,
    integrationBase: commit.parents[0].sha,
    integrationTree: commit.tree.sha,
  };
}

function belongsToPull(candidate, pullNumber) {
  if (pullNumber === undefined) return true;
  if (!Number.isSafeInteger(pullNumber) || pullNumber <= 0) return false;
  if (!Array.isArray(candidate?.pull_requests)) return false;
  return candidate.pull_requests.length === 0
    || candidate.pull_requests.some((pull) => pull?.number === pullNumber);
}
