const LANES = ["check", "products-proof"];
const ADMISSION_EVIDENCE = "admission-evidence";
const REQUIRED_ADMISSION = "admission";

function exactArray(value, label) {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) throw new Error(`${label} must be a string array`);
  return value;
}

export function laneInputsUnchanged(plan, lane) {
  if (!plan || typeof plan !== "object") throw new Error("reconciliation impact plan is required");
  if (typeof plan.pages_products_required !== "boolean" || typeof plan.full_fallback !== "boolean") throw new Error("reconciliation impact plan has unknown boolean fields");
  if (lane === "products-proof") return !plan.full_fallback && !plan.pages_products_required;
  if (lane !== "check") throw new Error(`unknown reconciliation lane ${lane}`);
  const shards = plan.workspace_shards;
  if (!shards || typeof shards !== "object" || Array.isArray(shards) || Object.values(shards).some((required) => typeof required !== "boolean")) {
    throw new Error("reconciliation workspace shard plan is malformed");
  }
  const controllerProofs = exactArray(plan.ci_controller_proofs, "controller proofs");
  const commandProofs = exactArray(plan.repository_command_proofs, "repository command proofs");
  for (const field of ["esp32_required", "browser_required", "conduitos_required", "conduitos_aarch64_product_required"]) {
    if (typeof plan[field] !== "boolean") throw new Error(`reconciliation impact plan has unknown ${field}`);
  }
  return !plan.full_fallback
    && controllerProofs.length === 0
    && commandProofs.length === 0
    && !plan.esp32_required
    && !plan.browser_required
    && !plan.conduitos_required
    && !plan.conduitos_aarch64_product_required
    && Object.values(shards).every((required) => !required);
}

function exactIdentity(value, label) {
  if (!/^[0-9a-f]{40,64}$/.test(value ?? "")) throw new Error(`${label} must be an exact Git identity`);
  return value;
}

function headers(token) {
  if (!token) throw new Error("GitHub token is required");
  return { Accept: "application/vnd.github+json", Authorization: `Bearer ${token}`, "X-GitHub-Api-Version": "2022-11-28" };
}

function validate(repository, pullNumber, candidateSha) {
  if (!/^[^/]+\/[^/]+$/.test(repository ?? "")) throw new Error("repository must be owner/name");
  if (!Number.isSafeInteger(pullNumber) || pullNumber < 1) throw new Error("pull number must be positive");
  exactIdentity(candidateSha, "candidate SHA");
}

export function successfulLaneEvidence(checkRuns, lane) {
  if (!LANES.includes(lane)) throw new Error(`unknown reconciliation lane ${lane}`);
  return checkRuns.filter((check) => check.name === lane
    && check.status === "completed"
    && check.conclusion === "success"
    && check.app?.slug === "github-actions"
    && Number.isSafeInteger(check.id)
    && check.id > 0
    && /^https:\/\//.test(check.details_url ?? ""))
    .sort((left, right) => (right.id ?? 0) - (left.id ?? 0))[0] ?? null;
}

export function successfulAdmissionEvidence(checkRuns, candidateSha, baseSha, integrationSha) {
  const externalId = `conduit.current-controller-reconciliation/v3:${candidateSha}:${baseSha}:${integrationSha}`;
  return checkRuns.filter((check) => check.name === ADMISSION_EVIDENCE
    && check.status === "completed"
    && check.conclusion === "success"
    && check.app?.slug === "github-actions"
    && check.external_id === externalId
    && Number.isSafeInteger(check.id)
    && check.id > 0
    && /^https:\/\//.test(check.details_url ?? ""))
    .sort((left, right) => (right.id ?? 0) - (left.id ?? 0))[0] ?? null;
}

export async function resolveCandidateRequest({ repository, pullNumber, candidateSha, baseSha, integrationSha, reconciliationPlan, token, request = fetch }) {
  validate(repository, pullNumber, candidateSha);
  exactIdentity(baseSha, "base SHA");
  exactIdentity(integrationSha, "integration SHA");
  const requestHeaders = headers(token);
  const pullResponse = await request(`https://api.github.com/repos/${repository}/pulls/${pullNumber}`, { headers: requestHeaders });
  if (!pullResponse.ok) throw new Error(`resolve pull request failed: HTTP ${pullResponse.status}`);
  const pull = await pullResponse.json();
  if (pull?.state !== "open") throw new Error(`pull request #${pullNumber} is not open`);
  if (pull?.head?.sha !== candidateSha) throw new Error(`candidate ${candidateSha} is not the current head of pull request #${pullNumber}`);
  if (pull?.base?.repo?.full_name !== repository) throw new Error("pull request target repository does not match the requested repository");
  const checkRuns = [];
  for (let page = 1; page <= 10; page += 1) {
    const query = new URLSearchParams({ filter: "all", per_page: "100", page: String(page) });
    const response = await request(`https://api.github.com/repos/${repository}/commits/${candidateSha}/check-runs?${query}`, { headers: requestHeaders });
    if (!response.ok) throw new Error(`list candidate checks failed: HTTP ${response.status}`);
    const body = await response.json();
    if (!Array.isArray(body.check_runs)) throw new Error("candidate check response is malformed");
    checkRuns.push(...body.check_runs);
    if (body.check_runs.length < 100) break;
  }
  const admitted = successfulAdmissionEvidence(checkRuns, candidateSha, baseSha, integrationSha);
  if (admitted) {
    const evidence = { checkRunId: admitted.id, detailsUrl: admitted.details_url };
    return {
      pullNumber, candidateSha, baseSha, integrationSha,
      lanes: Object.fromEntries(LANES.map((lane) => [lane, evidence])),
      laneInputsUnchanged: Object.fromEntries(LANES.map((lane) => [lane, true])),
      published: true,
    };
  }
  return {
    pullNumber, candidateSha, baseSha, integrationSha,
    lanes: Object.fromEntries(LANES.map((lane) => {
      if (!laneInputsUnchanged(reconciliationPlan, lane)) return [lane, null];
      const evidence = successfulLaneEvidence(checkRuns, lane);
      return [lane, evidence && { checkRunId: evidence.id, detailsUrl: evidence.details_url }];
    })),
    laneInputsUnchanged: Object.fromEntries(LANES.map((lane) => [lane, laneInputsUnchanged(reconciliationPlan, lane)])),
    published: false,
  };
}

export async function publishCandidateResults({ repository, candidateSha, baseSha, integrationSha, checkInherited, checkResult, checkEvidenceUrl, productsInherited, productsResult, productsEvidenceUrl, runUrl, publishRequiredAdmission = false, token, request = fetch }) {
  validate(repository, 1, candidateSha);
  exactIdentity(baseSha, "base SHA");
  exactIdentity(integrationSha, "integration SHA");
  const requestHeaders = { ...headers(token), "Content-Type": "application/json" };
  const lanes = [
    { name: "check", inherited: checkInherited, result: checkResult, evidenceUrl: checkEvidenceUrl },
    { name: "products-proof", inherited: productsInherited, result: productsResult, evidenceUrl: productsEvidenceUrl },
  ];
  const conclusion = lanes.every((lane) => lane.inherited || lane.result === "success") ? "success" : "failure";
  if (conclusion !== "success") {
    const refused = lanes.filter((lane) => !lane.inherited && lane.result !== "success").map((lane) => `${lane.name}=${lane.result}`).join(", ");
    throw new Error(`refuse admission because reconciliation did not succeed: ${refused}`);
  }
  const laneSummary = lanes.map((lane) => {
    if (lane.inherited) return `${lane.name}: inherited exact success from ${lane.evidenceUrl}`;
    return `${lane.name}: current-controller execution ${lane.result}`;
  }).join("\n");
  const evidenceBody = {
    name: ADMISSION_EVIDENCE, head_sha: candidateSha, status: "completed", conclusion,
    external_id: `conduit.current-controller-reconciliation/v3:${candidateSha}:${baseSha}:${integrationSha}`,
    details_url: runUrl,
    output: {
      title: "Exact candidate reconciliation admitted",
      summary: `Current-controller admission for unchanged candidate ${candidateSha}.\n\nBase: ${baseSha}\nProspective integration: ${integrationSha}\n\n${laneSummary}\n\nRun: ${runUrl}`,
    },
  };
  const bodies = [evidenceBody];
  if (publishRequiredAdmission) {
    bodies.push({
      ...evidenceBody,
      name: REQUIRED_ADMISSION,
      external_id: `conduit.required-admission/v1:${candidateSha}:${baseSha}:${integrationSha}`,
      output: {
        ...evidenceBody.output,
        title: "Exact candidate admission gate",
      },
    });
  }
  const published = [];
  for (const body of bodies) {
    const response = await request(`https://api.github.com/repos/${repository}/check-runs`, { method: "POST", headers: requestHeaders, body: JSON.stringify(body) });
    if (!response.ok) throw new Error(`publish ${body.name} check failed: HTTP ${response.status}`);
    published.push({ name: body.name, conclusion, checkRunId: (await response.json()).id });
  }
  return published;
}

async function appendOutput(name, value) {
  if (!process.env.GITHUB_OUTPUT) throw new Error("GITHUB_OUTPUT is required");
  const { appendFile } = await import("node:fs/promises");
  await appendFile(process.env.GITHUB_OUTPUT, `${name}=${value}\n`);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  if (process.argv[2] === "resolve") {
    const { readFile } = await import("node:fs/promises");
    const reconciliationPlan = JSON.parse(await readFile(process.env.CONDUIT_RECONCILIATION_PLAN, "utf8"));
    const result = await resolveCandidateRequest({
      repository: process.env.GITHUB_REPOSITORY,
      pullNumber: Number(process.env.CONDUIT_PR_NUMBER),
      candidateSha: process.env.CONDUIT_CANDIDATE_SHA,
      baseSha: process.env.CONDUIT_BASE_SHA,
      integrationSha: process.env.CONDUIT_INTEGRATION_SHA,
      reconciliationPlan,
      runUrl: process.env.CONDUIT_RUN_URL,
      token: process.env.GH_TOKEN,
    });
    await appendOutput("base_sha", result.baseSha);
    await appendOutput("integration_sha", result.integrationSha);
    for (const lane of LANES) {
      const prefix = lane === "check" ? "check" : "products";
      await appendOutput(`${prefix}_inherited`, String(Boolean(result.lanes[lane])));
      await appendOutput(`${prefix}_evidence_url`, result.lanes[lane]?.detailsUrl ?? "");
    }
    await appendOutput("published", String(result.published));
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  } else if (process.argv[2] === "publish") {
    const published = await publishCandidateResults({
      repository: process.env.GITHUB_REPOSITORY, candidateSha: process.env.CONDUIT_CANDIDATE_SHA,
      baseSha: process.env.CONDUIT_BASE_SHA, integrationSha: process.env.CONDUIT_INTEGRATION_SHA,
      checkInherited: process.env.CONDUIT_CHECK_INHERITED === "true", checkResult: process.env.CONDUIT_CHECK_RESULT, checkEvidenceUrl: process.env.CONDUIT_CHECK_EVIDENCE_URL,
      productsInherited: process.env.CONDUIT_PRODUCTS_INHERITED === "true", productsResult: process.env.CONDUIT_PRODUCTS_RESULT, productsEvidenceUrl: process.env.CONDUIT_PRODUCTS_EVIDENCE_URL,
      publishRequiredAdmission: process.env.CONDUIT_PUBLISH_REQUIRED_ADMISSION === "true",
      runUrl: process.env.CONDUIT_RUN_URL, token: process.env.GH_TOKEN,
    });
    process.stdout.write(`${JSON.stringify({ schema: "conduit.ci.current-controller-reconciliation/v1", candidate_sha: process.env.CONDUIT_CANDIDATE_SHA, published }, null, 2)}\n`);
  } else {
    throw new Error("usage: reconcile-candidate-request.mjs resolve|publish");
  }
}
