// Admission owns release creation. dev itself is the coalesced next batch;
// an Actions pending run is not a durable repair queue.
export async function requestRelease({ api, git, isAncestor, mergedTree, repository, integratedSha = "" }) {
  const releases = pulls => pulls.filter(pull => pull.base.ref === "main" &&
    pull.head.repo?.full_name === repository && /^release\/[0-9a-f]{40}$/.test(pull.head.ref));
  const blocker = pulls => {
    const open = releases(pulls);
    if (open.length) return { status: "release-open", numbers: open.map(pull => pull.number) };
    const syncs = pulls.filter(pull => pull.base.ref === "dev" &&
      pull.head.repo?.full_name === repository && pull.head.ref.startsWith("sync-release/"));
    if (syncs.length) return { status: "sync-pending", numbers: syncs.map(pull => pull.number) };
    return null;
  };
  const readPulls = async () => api("/pulls?state=open&per_page=100");
  let blocked = blocker(await readPulls());
  if (blocked) return blocked;

  // Events are wake-ups, not a FIFO of commits to release. Select fresh
  // repository evidence; a delayed success event cannot resurrect an old batch.
  const response = await api("/actions/workflows/dev-integration.yml/runs?branch=dev&per_page=100");
  const completed = response.workflow_runs.filter(run => run.status === "completed" &&
    ["push", "workflow_dispatch"].includes(run.event) && run.head_branch === "dev" &&
    run.head_repository?.full_name === repository &&
    !["cancelled", "skipped"].includes(run.conclusion));
  completed.sort((a, b) => Date.parse(b.created_at) - Date.parse(a.created_at) || b.id - a.id);
  const integrated = completed[0];
  if (!integrated || integrated.conclusion !== "success") return { status: "integration-required" };
  const sha = integrated.head_sha;
  if (!/^[0-9a-f]{40}$/.test(sha)) throw new Error("invalid integrated SHA");
  if (integratedSha && integratedSha !== sha) return { status: "superseded-integration" };

  git("fetch", "--no-tags", "origin", "main", "dev", sha);
  const main = git("rev-parse", "origin/main");
  if (!isAncestor(sha, "origin/dev")) return { status: "superseded-integration" };
  const tree = git("rev-parse", `${sha}^{tree}`);
  if (isAncestor(sha, main) || tree === git("rev-parse", `${main}^{tree}`)) {
    return { status: "already-current" };
  }
  // A successful dev run predating release repair must not drop those fixes.
  if (mergedTree(main, sha) !== tree) return { status: "release-fixes-not-integrated" };
  const branch = `release/${sha}`;
  const history = await api(`/pulls?state=closed&base=main&head=${repository.split("/")[0]}:${branch}&per_page=100`);
  if (history.length) return { status: "previous-release-requires-review", numbers: history.map(pull => pull.number) };

  // Recheck after the slower evidence reads, before either external write.
  blocked = blocker(await readPulls());
  if (blocked) return blocked;
  git("fetch", "--no-tags", "origin", "main");
  if (git("rev-parse", "origin/main") !== main) return { status: "main-moved" };
  git("push", "origin", `${sha}:refs/heads/${branch}`);
  const pull = await api("/pulls", "POST", {
    base: "main", head: branch, title: "Release the integrated development batch",
    body: "This batch owns the release until accepted or explicitly abandoned. New development accumulates for the next batch. Repair failures on this branch only after the prior attempt is terminal; the exact final head must pass exhaustive promotion. Accepted fixes return to dev automatically.",
  });
  if (pull.head.sha !== sha || pull.head.ref !== branch || pull.base.ref !== "main") {
    throw new Error("created release does not match admitted head");
  }
  await api("/actions/workflows/monitor-trusted-pr.yml/dispatches", "POST", {
    ref: "main", inputs: { kind: "release", pr_number: String(pull.number), head_sha: sha },
  });
  return { status: "created", number: pull.number, url: pull.html_url, headSha: sha, integrationRun: integrated.id };
}
