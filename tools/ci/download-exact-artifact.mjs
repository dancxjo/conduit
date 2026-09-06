import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

export const SCHEMA = "conduit.ci.artifact-transport/v1";
const MAX_ATTEMPTS = 3;
const BACKOFF_MS = [1_000, 2_000];

function header(response, name) {
  return response.headers?.get?.(name) ?? null;
}

export function classifyHttpFailure(stage, response, body) {
  if (response.status === 429 || response.status >= 500) {
    return { transient: true, reason: `${stage}-http-${response.status}` };
  }
  const intermediary403 =
    response.status === 403 &&
    (body.toLowerCase().includes("intermediary") ||
      header(response, "x-ms-error-code") !== null);
  if (intermediary403) {
    return { transient: true, reason: `${stage}-intermediary-403` };
  }
  return { transient: false, reason: `${stage}-http-${response.status}` };
}

function exactArtifact(payload, name) {
  if (!payload || !Array.isArray(payload.artifacts)) {
    throw new Error("artifact list response has an unknown schema");
  }
  const matches = payload.artifacts.filter(
    (artifact) => artifact?.name === name && artifact?.expired === false,
  );
  if (matches.length !== 1 || !Number.isSafeInteger(matches[0].id)) {
    throw new Error(`expected one live exact artifact named ${name}; found ${matches.length}`);
  }
  return matches[0];
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export async function downloadExactArtifact(options) {
  const {
    repository,
    runId,
    name,
    expectedDigest,
    token,
    fetchImpl = fetch,
    sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds)),
  } = options;
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
    throw new Error("artifact transport requires owner/repository identity");
  }
  if (!/^[1-9][0-9]*$/.test(String(runId))) {
    throw new Error("artifact transport requires a positive producer run id");
  }
  if (!name || !/^[0-9a-f]{64}$/.test(expectedDigest) || !token) {
    throw new Error("artifact transport requires exact name, digest, and token");
  }

  const headers = {
    Accept: "application/vnd.github+json",
    Authorization: `Bearer ${token}`,
    "X-GitHub-Api-Version": "2022-11-28",
  };
  const encodedName = encodeURIComponent(name);
  const listUrl = `https://api.github.com/repos/${repository}/actions/runs/${runId}/artifacts?name=${encodedName}&per_page=100`;
  const retries = [];

  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    let response;
    try {
      response = await fetchImpl(listUrl, { headers, redirect: "follow" });
    } catch (error) {
      retries.push(`list-network:${error.message}`);
      if (attempt === MAX_ATTEMPTS) throw error;
      await sleep(BACKOFF_MS[attempt - 1]);
      continue;
    }
    if (!response.ok) {
      const body = await response.text();
      const classification = classifyHttpFailure("list", response, body);
      if (!classification.transient || attempt === MAX_ATTEMPTS) {
        throw new Error(`artifact list failed permanently: ${classification.reason}`);
      }
      retries.push(classification.reason);
      await sleep(BACKOFF_MS[attempt - 1]);
      continue;
    }

    const artifact = exactArtifact(await response.json(), name);
    const downloadUrl = `https://api.github.com/repos/${repository}/actions/artifacts/${artifact.id}/zip`;
    try {
      response = await fetchImpl(downloadUrl, { headers, redirect: "follow" });
    } catch (error) {
      retries.push(`download-network:${error.message}`);
      if (attempt === MAX_ATTEMPTS) throw error;
      await sleep(BACKOFF_MS[attempt - 1]);
      continue;
    }
    if (!response.ok) {
      const body = await response.text();
      const classification = classifyHttpFailure("download", response, body);
      if (!classification.transient || attempt === MAX_ATTEMPTS) {
        throw new Error(`artifact download failed permanently: ${classification.reason}`);
      }
      retries.push(classification.reason);
      await sleep(BACKOFF_MS[attempt - 1]);
      continue;
    }

    const bytes = Buffer.from(await response.arrayBuffer());
    const actualDigest = sha256(bytes);
    if (actualDigest !== expectedDigest) {
      throw new Error(
        `artifact digest mismatch for ${name}: expected ${expectedDigest}, actual ${actualDigest}`,
      );
    }
    return { bytes, attempts: attempt, retries, artifactId: artifact.id, actualDigest };
  }
  throw new Error("artifact transport exhausted without a classified result");
}

async function main() {
  const resultPath = process.env.CONDUIT_ARTIFACT_RESULT;
  const provenance = {
    schema: SCHEMA,
    repository: process.env.GITHUB_REPOSITORY,
    run_id: process.env.CONDUIT_ARTIFACT_RUN_ID,
    artifact_name: process.env.CONDUIT_ARTIFACT_NAME,
  };
  try {
    const result = await downloadExactArtifact({
      repository: provenance.repository,
      runId: provenance.run_id,
      name: provenance.artifact_name,
      expectedDigest: process.env.CONDUIT_ARTIFACT_EXPECTED_DIGEST,
      token: process.env.GH_TOKEN,
    });
    await mkdir(dirname(process.env.CONDUIT_ARTIFACT_ARCHIVE), { recursive: true });
    await writeFile(process.env.CONDUIT_ARTIFACT_ARCHIVE, result.bytes);
    await writeFile(
      resultPath,
      `${JSON.stringify({
        ...provenance,
        status: "success",
        attempts: result.attempts,
        retries: result.retries,
        artifact_id: result.artifactId,
        digest: result.actualDigest,
      }, null, 2)}\n`,
    );
    await writeFile(process.env.GITHUB_OUTPUT, `attempts=${result.attempts}\n`, { flag: "a" });
    await writeFile(
      process.env.GITHUB_STEP_SUMMARY,
      `Artifact \`${provenance.artifact_name}\`: ${result.attempts} transport attempt(s); exact digest verified.\n`,
      { flag: "a" },
    );
  } catch (error) {
    if (resultPath) {
      await mkdir(dirname(resultPath), { recursive: true });
      await writeFile(
        resultPath,
        `${JSON.stringify({ ...provenance, status: "failure", reason: error.message }, null, 2)}\n`,
      );
    }
    if (process.env.GITHUB_STEP_SUMMARY) {
      await writeFile(
        process.env.GITHUB_STEP_SUMMARY,
        `Artifact transport \`${provenance.artifact_name}\`: failed before proof execution (${error.message}).\n`,
        { flag: "a" },
      );
    }
    throw error;
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
