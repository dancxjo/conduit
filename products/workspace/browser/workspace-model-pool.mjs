import { openBrowserSharedPool } from "../../../targets/browser/host/assets/browser-shared-pool.mjs";

/** Bind joined Hosts to one immutable planned worker envelope.
 *
 * This owner gathers current Host-authored observations, delegates selection
 * and occupation to the common Rust kernel, and returns the exact joined Line
 * for the selected realization. Provider invocation remains the caller's
 * ordinary remote-fragment responsibility.
 */
export function prepareWorkspaceModelPool({
  api, plan, localAdvertisement, joinedHosts, poolId,
  firstMemberNode = 256, play = 1, openPool = openBrowserSharedPool,
}) {
  const fragments = Array.isArray(plan?.fragments) ? plan.fragments : [];
  const fragmentIndex = fragments.findIndex((fragment) =>
    fragment.host_id === localAdvertisement?.host_id
      && fragment.boot_id === localAdvertisement?.boot_id
      && fragment.offer_generation === localAdvertisement?.offer_generation);
  const pool = fragmentIndex >= 0
    ? fragments[fragmentIndex].shared_pools?.find((candidate) => candidate.pool_id === poolId)
    : null;
  const envelope = pool?.realization_envelope;
  if (!pool || !Array.isArray(envelope) || envelope.length < 1 || envelope.length > 16
    || !Array.isArray(joinedHosts) || joinedHosts.length < envelope.length) {
    throw new Error("Workspace model pool is absent or outside its finite Host bound");
  }
  const lines = envelope.map((realization) => {
    const matches = joinedHosts.filter((joined) => joined?.host_id === realization.host_id
      && joined.boot_id === realization.boot_id
      && joined.advertisement?.host_id === realization.host_id
      && joined.advertisement?.boot_id === realization.boot_id
      && joined.advertisement?.offer_generation === realization.offer_generation
      && joined.line?.schema === "conduit.creche/joined-host-line@1");
    if (matches.length !== 1) {
      throw new Error("planned model realization has no unique exact joined Host Line");
    }
    return matches[0];
  });
  const runtime = openPool({
    api, plan, fragmentIndex, poolId, firstMemberNode, play,
  });
  let closed = false;
  const current = () => {
    if (closed) throw new Error("Workspace model pool is closed");
  };
  const observe = async () => {
    current();
    return await Promise.all(envelope.map((realization, index) =>
      lines[index].line.observeLocalModelPool(realization)));
  };
  return Object.freeze({
    planId: plan.plan_id,
    poolId,
    envelope: Object.freeze([...envelope]),
    async observe() {
      return Object.freeze(await observe());
    },
    async admit({ operationId, key, selectionSignId }) {
      const observations = await observe();
      const selection = runtime.admit({ operationId, key, observations, selectionSignId });
      if (selection.disposition === "refused") {
        return Object.freeze({ selection, observations: Object.freeze(observations) });
      }
      const index = selection.member.realization;
      if (!Number.isSafeInteger(index) || index < 0 || index >= envelope.length) {
        throw new Error("kernel selected a realization outside the immutable envelope");
      }
      return Object.freeze({
        selection,
        realization: envelope[index],
        joined: lines[index],
        observations: Object.freeze(observations),
      });
    },
    activate(admission) {
      current();
      return runtime.trigger(admission?.selection?.member);
    },
    failPreparation(admission) {
      current();
      return runtime.failPreparation(admission?.selection?.member);
    },
    release(admission) {
      current();
      return runtime.release(admission?.selection?.member);
    },
    async providerLost({ admission, operationId, lossSignId }) {
      const observations = await observe();
      return Object.freeze({
        loss: runtime.providerLost({
          operationId, member: admission?.selection?.member, observations, lossSignId,
        }),
        observations: Object.freeze(observations),
      });
    },
    close() {
      if (closed) return;
      closed = true;
      runtime.close();
    },
  });
}
