import { openBrowserSharedPool } from "../../../targets/browser/host/assets/browser-shared-pool.mjs";
import { openBrowserPoolMemberClient } from "../../../targets/browser/host/assets/browser-pool-member-client.mjs";

/** Bind joined Hosts to one immutable planned worker envelope.
 *
 * This owner gathers current Host-authored observations, delegates selection
 * and occupation to the common Rust kernel, then drives the selected member
 * only through its Plan-derived semantic sessions.
 */
export function prepareWorkspaceModelPool({
  api, plan, localAdvertisement, joinedHosts, poolId,
  firstMemberNode = 256, play = 1, openPool = openBrowserSharedPool,
  openMemberClient = openBrowserPoolMemberClient,
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
  const capabilities = [];
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
    const offered = matches[0].advertisement?.capabilities?.filter((capability) =>
      capability?.capability_id === realization.capability_id
        && capability.implementation_id === realization.implementation_id
        && capability.artifact_id === realization.artifact_id) ?? [];
    if (offered.length !== 1 || offered[0].kind_id !== "ai/generate-text"
      || offered[0].kind_contract_revision !== "conduit.ai/generate-text@1") {
      throw new Error("planned model realization lacks one exact ai/generate-text front/back");
    }
    capabilities.push(offered[0]);
    return matches[0];
  });
  const runtime = openPool({
    api, plan, fragmentIndex, poolId, firstMemberNode, play,
  });
  let closed = false;
  const completed = [];
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
    async prepare(admission, consumerPlacementId) {
      current();
      const evidence = admission?.selection?.evidence;
      if (!evidence || admission?.joined?.line?.schema !== "conduit.creche/joined-host-line@1"
        || !pool.consumers?.includes(consumerPlacementId)) {
        throw new Error("pool member preparation requires exact selected evidence and consumer");
      }
      try {
        return await admission.joined.line.preparePoolMember({
          plan, selection: evidence, consumerPlacementId,
        });
      } catch (error) {
        runtime.failPreparation(admission.selection.member);
        throw error;
      }
    },
    async execute({ admission, preparation, consumerPlacementId, prompt }) {
      current();
      if (completed.length >= 16) {
        throw new Error("pool operation receipt capacity is exhausted");
      }
      const evidence = admission?.selection?.evidence;
      if (!evidence || !Array.isArray(preparation?.hello_frames)
        || admission?.joined?.line?.schema !== "conduit.creche/joined-host-line@1") {
        throw new Error("pool member execution requires exact preparation and joined Line");
      }
      const client = openMemberClient({
        api, plan, selection: evidence, consumerPlacementId,
        sessionHellos: preparation.hello_frames,
      });
      try {
        for (const frame of client.initialFrames) {
          await admission.joined.line.sendSessionFrame(frame);
        }
        const sessionCount = preparation.hello_frames.length;
        for (let index = 0; index < sessionCount; index += 1) {
          const ready = client.exchange(await admission.joined.line.receiveSessionFrame());
          if (ready.message !== "ready" || ready.responses.length !== 0) {
            throw new Error("pool member handshake returned unexpected session truth");
          }
        }
        await admission.joined.line.sendSessionFrame(client.offer(prompt));
        const maximumResponses = sessionCount * 4;
        for (let index = 0; index < maximumResponses; index += 1) {
          const exchanged = client.exchange(await admission.joined.line.receiveSessionFrame());
          for (const response of exchanged.responses) {
            await admission.joined.line.sendSessionFrame(response);
          }
          if (exchanged.result) {
            const execution = Object.freeze({
              planId: plan.plan_id,
              poolId,
              operationId: evidence.operation_id,
              realization: admission.selection.member.realization,
              hostId: admission.realization.host_id,
              bootId: admission.realization.boot_id,
              capabilityId: admission.realization.capability_id,
              member: admission.selection.member,
              populationAtAdmission: admission.selection.population,
              selectionSignId: evidence.sign_id,
              observationSignIds: Object.freeze([...evidence.observation_sign_ids]),
              result: exchanged.result,
            });
            completed.push(Object.freeze({
              operation_id: execution.operationId,
              realization: execution.realization,
              host_id: execution.hostId,
              boot_id: execution.bootId,
              capability_id: execution.capabilityId,
              member_key: execution.member.key,
              member_slot: execution.member.slot,
              member_epoch: execution.member.epoch,
              member_node: execution.member.node,
              member_play: execution.member.play,
              population_at_admission: execution.populationAtAdmission,
              selection_sign_id: execution.selectionSignId,
              observation_sign_ids: execution.observationSignIds,
              result_bytes: execution.result.byteLength,
              disposition: "Completed",
            }));
            return execution;
          }
          if (["failed", "cancelled", "terminal"].includes(exchanged.message)) {
            throw new Error(`pool member terminated without a result: ${exchanged.message}`);
          }
        }
        throw new Error("pool member exceeded its finite response bound");
      } finally {
        client.close();
      }
    },
    receipt({ bodyId, wakeId, sourceDocumentId, checkedFormId, expandedFormId, playId }) {
      current();
      const identities = {
        body_id: bodyId, wake_id: wakeId, source_document_id: sourceDocumentId,
        checked_form_id: checkedFormId, expanded_form_id: expandedFormId, play_id: playId,
      };
      if (Object.values(identities).some((value) =>
        typeof value !== "string" || value.length < 1 || value.length > 192 || /\s/.test(value))) {
        throw new Error("pool receipt identity is absent or outside its finite bound");
      }
      return Object.freeze({
        schema: "conduit.proof/live-local-model-pool@1",
        ...identities,
        plan_id: plan.plan_id,
        pool_id: poolId,
        member_kind_id: capabilities[0].kind_id,
        member_kind_contract_revision: capabilities[0].kind_contract_revision,
        realization_envelope: Object.freeze(envelope.map((realization) => Object.freeze({
          host_id: realization.host_id,
          boot_id: realization.boot_id,
          offer_generation: realization.offer_generation,
          capability_id: realization.capability_id,
          implementation_id: realization.implementation_id,
          artifact_id: realization.artifact_id,
          member_capacity: realization.member_capacity,
          resources: realization.resources,
        }))),
        operations: Object.freeze([...completed]),
        prompt_content_retained: false,
        physical_evidence: false,
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
