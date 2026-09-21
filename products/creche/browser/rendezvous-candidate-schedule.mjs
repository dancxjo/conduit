export class RemoteCandidateScheduleError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "RemoteCandidateScheduleError";
    this.code = code;
  }
}

export async function openOrderedRemoteCandidates(decoded, {
  signal,
  openCandidate,
  now = () => Date.now(),
} = {}) {
  if (!Array.isArray(decoded?.candidates) || decoded.candidates.length < 1
    || typeof openCandidate !== "function") {
    throw new RemoteCandidateScheduleError(
      "InvalidDescriptor", "remote rendezvous omitted its finite candidate schedule",
    );
  }
  const evidence = [];
  for (const candidate of decoded.candidates) {
    if (candidate.expires_at_millis <= now()) {
      evidence.push(attemptEvidence(candidate, 0, "expired"));
      continue;
    }
    if (!candidate.supported) {
      evidence.push(attemptEvidence(candidate, 0, "unsupported-line-family"));
      continue;
    }
    for (let attempt = 1; attempt <= candidate.maximum_attempts; attempt += 1) {
      requireCurrent(signal);
      try {
        const line = await waitForBoundedPromise(
          Promise.resolve(openCandidate(candidate, signal)),
          signal,
          candidate.attempt_timeout_millis,
        );
        requireLine(line);
        evidence.push(attemptEvidence(candidate, attempt, "connected"));
        return Object.freeze({
          line,
          selected: candidate,
          evidence: Object.freeze(evidence),
        });
      } catch (error) {
        evidence.push(attemptEvidence(candidate, attempt, attemptOutcome(error)));
      }
      if (candidate.expires_at_millis <= now()) break;
    }
  }
  throw new RemoteCandidateScheduleError(
    "LineUnavailable",
    `remote rendezvous candidates exhausted: ${JSON.stringify(evidence)}`,
  );
}

function requireLine(line) {
  if (!line || typeof line.sendBytes !== "function"
    || typeof line.receiveBytes !== "function" || typeof line.close !== "function") {
    throw new RemoteCandidateScheduleError(
      "LineFailed", "rendezvous candidate did not provide the bounded Line contract",
    );
  }
}

function attemptEvidence(candidate, attempt, outcome) {
  return Object.freeze({
    candidate_id: candidate.candidate_id,
    attempt,
    timeout_millis: candidate.attempt_timeout_millis ?? 0,
    outcome,
  });
}

function attemptOutcome(error) {
  if (error?.code === "UnsupportedLineFamily") return "unsupported-line-family";
  if (error?.code === "LineTimeout") return "timed-out";
  if (error?.code === "TransportAuthentication") return "authentication-refused";
  if (error?.code === "PeerBinding") return "peer-binding-refused";
  if (error?.code === "LinePressure") return "pressure-refused";
  return "route-unavailable";
}

function requireCurrent(signal) {
  if (signal?.aborted) throw signal.reason ?? new DOMException("Aborted", "AbortError");
}

function waitForBoundedPromise(promise, signal, timeoutMillis) {
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (callback, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      signal?.removeEventListener("abort", aborted);
      callback(value);
    };
    const aborted = () => finish(
      reject, signal.reason ?? new DOMException("Aborted", "AbortError"),
    );
    const timer = setTimeout(() => finish(
      reject,
      new RemoteCandidateScheduleError(
        "LineTimeout", "rendezvous candidate did not answer within its admitted time",
      ),
    ), timeoutMillis);
    promise.then((value) => finish(resolve, value), (error) => finish(reject, error));
    signal?.addEventListener("abort", aborted, { once: true });
    if (signal?.aborted) aborted();
  });
}
