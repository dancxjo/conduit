const ACTIVE = new Set(["queued", "in_progress", "pending", "waiting"]);
const TERMINAL = new Set(["completed"]);

export const DEFAULT_NO_PROGRESS_MS = 15 * 60 * 1_000;
export const DEFAULT_MAX_ELAPSED_MS = 45 * 60 * 1_000;

function time(value) {
  const parsed = typeof value === "number" ? value : Date.parse(value ?? "");
  return Number.isFinite(parsed) ? parsed : 0;
}

function orderedAttempts(candidate) {
  return [...(candidate.attempts ?? [])].sort((left, right) =>
    time(left.createdAt) - time(right.createdAt) || left.id - right.id,
  );
}

function latestAttempt(candidate) {
  return orderedAttempts(candidate).at(-1);
}

function failedJobs(attempt) {
  return (attempt?.jobs ?? [])
    .filter((job) => job.conclusion === "failure")
    .map((job) => ({
      name: job.name,
      id: job.id ?? null,
      url: job.url ?? null,
      conclusion: "failure",
    }));
}

function materiallyStarted(attempt) {
  return attempt?.status === "in_progress" || (attempt?.jobs ?? []).some((job) =>
    job.status === "in_progress" || TERMINAL.has(job.status) || time(job.startedAt) > 0,
  );
}

function lastProgressAt(attempt) {
  return Math.max(
    time(attempt?.createdAt),
    time(attempt?.updatedAt),
    ...(attempt?.jobs ?? []).flatMap((job) => [time(job.startedAt), time(job.completedAt)]),
  );
}

function evidence(candidate, attempt, jobs = failedJobs(attempt)) {
  return {
    prNumber: candidate.prNumber,
    headSha: candidate.headSha,
    branch: candidate.branch,
    runId: attempt?.id ?? null,
    runUrl: attempt?.url ?? null,
    failedJobs: jobs,
  };
}

export function classifyCandidate(candidate, options = {}) {
  const now = options.now ?? Date.now();
  const noProgressMs = options.noProgressMs ?? DEFAULT_NO_PROGRESS_MS;
  const maxElapsedMs = options.maxElapsedMs ?? DEFAULT_MAX_ELAPSED_MS;
  const attempts = orderedAttempts(candidate);
  const attempt = attempts.at(-1);
  if (!attempt) {
    return { state: "queued", attempt: null, evidence: evidence(candidate, null) };
  }

  const failures = failedJobs(attempt);
  const priorBadSameHead = attempts.slice(0, -1).some((prior) =>
    prior.headSha === candidate.headSha
      && (prior.conclusion === "failure" || failedJobs(prior).length > 0),
  );
  if (ACTIVE.has(attempt.status) && priorBadSameHead && !attempt.explicitRepair) {
    return {
      state: "running-bad",
      reason: "known-bad-restart",
      attempt,
      evidence: evidence(candidate, attempt, failures),
    };
  }
  if (failures.length > 0 || attempt.conclusion === "failure") {
    return {
      state: "running-bad",
      reason: "decisive-failure",
      attempt,
      evidence: evidence(candidate, attempt, failures),
    };
  }
  if (attempt.status === "completed" && attempt.conclusion === "success") {
    return { state: "passed", attempt, evidence: evidence(candidate, attempt) };
  }
  if (attempt.status === "completed" && attempt.conclusion === "cancelled") {
    const state = attempt.cancelReason === "bad" ? "cancelled-bad" : "cancelled-superseded";
    return { state, attempt, evidence: evidence(candidate, attempt) };
  }
  if (!ACTIVE.has(attempt.status)) {
    return {
      state: "running-bad",
      reason: "unsupported-run-state",
      attempt,
      evidence: evidence(candidate, attempt),
    };
  }

  const started = materiallyStarted(attempt);
  const elapsed = now - time(attempt.createdAt);
  const quiet = now - lastProgressAt(attempt);
  if ((started && (elapsed >= maxElapsedMs || quiet >= noProgressMs))
      || (!started && quiet >= noProgressMs)) {
    return {
      state: "stuck",
      reason: elapsed >= maxElapsedMs ? "elapsed-limit" : "no-progress",
      attempt,
      evidence: evidence(candidate, attempt),
    };
  }
  return {
    state: started ? "running-healthy" : "queued",
    attempt,
    evidence: evidence(candidate, attempt),
  };
}

function newest(left, right) {
  return time(left.createdAt) - time(right.createdAt)
    || (left.prNumber ?? 0) - (right.prNumber ?? 0)
    || (latestAttempt(left)?.id ?? 0) - (latestAttempt(right)?.id ?? 0);
}

export function decideReleaseLane(snapshot, options = {}) {
  const classified = (snapshot.candidates ?? []).map((candidate) => ({
    candidate,
    classification: classifyCandidate(candidate, { ...options, now: snapshot.now ?? options.now }),
  }));
  const actions = {
    preserve: [],
    cancel: [],
    close: [],
    start: null,
    finalize: [],
    escalate: [],
  };

  for (const item of classified) {
    const { candidate, classification } = item;
    if (classification.state === "passed") {
      actions.finalize.push(classification.evidence);
    } else if (classification.state === "running-bad") {
      if (classification.attempt && classification.attempt.status !== "completed") {
        actions.cancel.push({
          ...classification.evidence,
          state: "cancelled-bad",
          reason: classification.reason,
        });
      }
    }
  }

  const laneOwners = classified
    .filter(({ classification }) => ["running-healthy", "stuck"].includes(classification.state))
    .sort((left, right) => time(left.classification.attempt.createdAt)
      - time(right.classification.attempt.createdAt));
  const owner = laneOwners[0] ?? null;
  if (owner) {
    actions.preserve.push({
      ...owner.classification.evidence,
      state: owner.classification.state,
    });
  }
  for (const extra of laneOwners.slice(1)) {
    actions.cancel.push({
      ...extra.classification.evidence,
      state: "cancelled-superseded",
      reason: "multiple-running-attempts",
    });
  }

  const queued = classified
    .filter(({ classification }) => classification.state === "queued")
    .sort((left, right) => newest(left.candidate, right.candidate));
  const retainedQueue = queued.at(-1) ?? null;
  for (const obsolete of queued.slice(0, -1)) {
    actions.cancel.push({
      ...obsolete.classification.evidence,
      state: "cancelled-superseded",
      reason: "newer-queued-candidate",
    });
    actions.close.push({
      prNumber: obsolete.candidate.prNumber,
      headSha: obsolete.candidate.headSha,
      reason: "newer-queued-candidate",
    });
  }

  const newestLive = classified
    .filter(({ classification }) => !["cancelled-superseded", "cancelled-bad"].includes(classification.state))
    .sort((left, right) => newest(left.candidate, right.candidate))
    .at(-1);
  if (newestLive) {
    for (const obsolete of classified.filter(({ classification, candidate }) =>
      classification.state === "cancelled-superseded"
        && newest(candidate, newestLive.candidate) < 0,
    )) {
      actions.close.push({
        prNumber: obsolete.candidate.prNumber,
        headSha: obsolete.candidate.headSha,
        reason: "newer-queued-candidate",
      });
    }
  }
  if (!owner && retainedQueue) {
    actions.start = retainedQueue.classification.evidence;
  } else if (retainedQueue) {
    actions.preserve.push({
      ...retainedQueue.classification.evidence,
      state: "queued",
    });
  }

  for (const stuck of classified.filter(({ classification }) => classification.state === "stuck")) {
    const key = `release-stuck:${stuck.classification.evidence.runId}`;
    if (!(snapshot.escalations ?? []).includes(key)) {
      actions.escalate.push({
        key,
        reason: stuck.classification.reason,
        ...stuck.classification.evidence,
      });
    }
  }

  return {
    schema: "conduit.ci/release-lane-decision@1",
    classifications: classified.map(({ candidate, classification }) => ({
      prNumber: candidate.prNumber,
      headSha: candidate.headSha,
      runId: classification.attempt?.id ?? null,
      state: classification.state,
      reason: classification.reason ?? null,
    })),
    actions,
  };
}
