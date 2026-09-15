const SCHEMA = "conduit.workspace/body-invitation@1";
const CHANNEL_PREFIX = "conduit.workspace/body-admission/";
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

export function readBodyInvitation(location) {
  const value = new URLSearchParams(location.hash.slice(1)).get("body-invitation");
  if (!value) return null;
  if (value.length > 8192) throw new Error("Body invitation exceeds its portable bound");
  try {
    const padding = "=".repeat((4 - value.length % 4) % 4);
    const bytes = Uint8Array.from(atob(value.replaceAll("-", "+").replaceAll("_", "/") + padding), character => character.charCodeAt(0));
    const artifact = JSON.parse(decoder.decode(bytes));
    if (artifact?.schema !== SCHEMA || !artifact.claim || !Array.isArray(artifact.secret) || artifact.secret.length !== 32) {
      throw new Error("Body invitation has an unsupported shape");
    }
    return Object.freeze(artifact);
  } catch (error) {
    throw new Error(`Body invitation is malformed: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function invitationUrl(location, artifact) {
  const bytes = encoder.encode(JSON.stringify(artifact));
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  const value = btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
  const url = new URL(location.href);
  url.search = "";
  url.hash = new URLSearchParams({ "body-invitation": value }).toString();
  if (url.href.length > 8192) throw new Error("Body invitation exceeds its portable bound");
  return url.href;
}

export function openWorkspaceMembership({ root, session, host, invitation, beforeAdmission, onChanged, onFailure }) {
  const panel = root.querySelector("#workspace-membership");
  const content = panel.querySelector("[data-membership-content]");
  const openButton = root.querySelector("[data-open-membership]");
  const inviteButton = root.querySelector("[data-invite-host]");
  const closeButton = root.querySelector("[data-close-membership]");
  const channels = new Set();
  let open = Boolean(invitation);

  const close = () => {
    if (invitation && !session.current()) return;
    open = false;
    panel.hidden = true;
    openButton?.setAttribute("aria-expanded", "false");
    onChanged();
    openButton?.focus();
  };
  closeButton.addEventListener("click", close);
  openButton.addEventListener("click", () => { open = true; render(); onChanged(); panel.querySelector("h2").focus(); });
  inviteButton.addEventListener("click", () => { open = true; renderInvite().catch(onFailure); onChanged(); });

  function render() {
    panel.hidden = !open;
    openButton.hidden = !session.current();
    inviteButton.hidden = !session.current();
    openButton.setAttribute("aria-expanded", String(open));
    if (!open) return;
    if (invitation && !session.current()) { renderJoin(invitation); return; }
    const evidence = session.evidence()?.evidence;
    if (!evidence) return;
    content.replaceChildren();
    const intro = document.createElement("p");
    intro.className = "membership-intro";
    intro.textContent = `${evidence.friendly_name} has ${evidence.membership.parts.length} retained Part${evidence.membership.parts.length === 1 ? "" : "s"}. Presence is current observation, not authority.`;
    const list = document.createElement("div"); list.className = "member-list";
    for (const part of evidence.membership.parts) {
      const item = document.createElement("article"); item.className = "member-card";
      const local = part.current?.host_id === host.hostId && part.current?.boot_id === host.bootId;
      const title = document.createElement("h3"); title.textContent = local ? "This browser" : "Body Part";
      const state = document.createElement("p");
      state.textContent = `${part.state.toLowerCase()} · ${part.current ? "present" : "offline"}${local ? " · local Host" : ""}`;
      const exact = document.createElement("details"), summary = document.createElement("summary"), pre = document.createElement("pre");
      summary.textContent = "Exact membership evidence"; pre.textContent = JSON.stringify(part, null, 2); exact.append(summary, pre);
      item.append(title, state, exact); list.append(item);
    }
    const action = document.createElement("button"); action.type = "button"; action.textContent = "Invite another Host";
    action.addEventListener("click", () => renderInvite().catch(onFailure));
    content.append(intro, list, action);
  }

  async function renderInvite() {
    const body = session.current();
    if (!body) throw new Error("A Body must exist before creating an invitation");
    const secret = crypto.getRandomValues(new Uint8Array(32));
    const nonce = crypto.getRandomValues(new Uint8Array(32));
    const claim = await session.createInvitation(secret, nonce);
    const artifact = Object.freeze({ schema: SCHEMA, claim, secret: Array.from(secret), body_name: body.friendly_name,
      offered_by: { part_id: body.here_part_id, host_id: host.hostId } });
    secret.fill(0); nonce.fill(0);
    const url = invitationUrl(globalThis.location, artifact);
    const channel = listenForJoin(claim);
    channels.add(channel);
    content.innerHTML = `<p class="membership-kicker">Invitation ready</p><h3>${escapeText(body.friendly_name)}</h3>
      <p>This single-use invitation offers admission to this Body for ten minutes. It does not authorize Forms or arbitrary effects. Keep this Workspace open while the other Host joins.</p>
      <label>Portable invitation link<textarea readonly data-invitation-link></textarea></label>
      <div class="membership-actions"><button type="button" data-copy-invitation>Copy invitation</button><button type="button" data-cancel-invitation>Back to members</button></div>
      <details><summary>Invitation details</summary><pre>${escapeText(JSON.stringify({ ...artifact, secret: "redacted bounded invitation capability" }, null, 2))}</pre></details>
      <p class="transport-note">Available now: another Host in this browser’s same-origin rendezvous. Remote internet rendezvous is not yet supported.</p>`;
    content.querySelector("[data-invitation-link]").value = url;
    content.querySelector("[data-copy-invitation]").addEventListener("click", async event => { await navigator.clipboard.writeText(url); event.currentTarget.textContent = "Copied"; });
    content.querySelector("[data-cancel-invitation]").addEventListener("click", () => { channel.close(); channels.delete(channel); render(); });
  }

  function listenForJoin(claim) {
    const channel = new BroadcastChannel(CHANNEL_PREFIX + claim.invitation_id);
    channel.addEventListener("message", async ({ data }) => {
      if (data?.type !== "join-request" || data.claim?.invitation_id !== claim.invitation_id) return;
      try {
        await beforeAdmission();
        const receipt = await session.admitInvitation(data.advertisement, data.proof);
        channel.postMessage({ type: "join-result", request_id: data.request_id, durable: receipt.durable });
        onChanged(); render();
      } catch (error) {
        channel.postMessage({ type: "join-refusal", request_id: data.request_id, message: error instanceof Error ? error.message : String(error) });
        onFailure(error);
      }
    });
    return channel;
  }

  function renderJoin(artifact) {
    try { session.inspectInvitation(artifact.claim); }
    catch (error) { onFailure(error); content.textContent = error.message; return; }
    content.innerHTML = `<p class="membership-kicker">Body invitation</p><h3>${escapeText(artifact.body_name ?? "Another Body")}</h3>
      <p>This invitation proposes a new Part for this exact browser Host. Joining records membership and authenticated presence separately. It grants no Form or effect authority.</p>
      <dl><dt>Body</dt><dd>${escapeText(artifact.claim.body_id)}</dd><dt>Offered by</dt><dd>${escapeText(artifact.offered_by?.host_id ?? "not disclosed")}</dd><dt>Expires</dt><dd>${new Date(artifact.claim.expires_at_millis).toLocaleString()}</dd></dl>
      <button type="button" data-join-body>Join this Body</button><p class="transport-note">The inviting Workspace must remain open in this browser’s same-origin rendezvous.</p>`;
    content.querySelector("[data-join-body]").addEventListener("click", async event => {
      event.currentTarget.disabled = true;
      try {
        const durable = await requestJoin(artifact);
        await session.openAdmitted(durable);
        history.replaceState(null, "", `${location.pathname}${location.search}`);
        open = true; onChanged(); render();
      } catch (error) { event.currentTarget.disabled = false; onFailure(error); }
    });
  }

  function requestJoin(artifact) {
    const channel = new BroadcastChannel(CHANNEL_PREFIX + artifact.claim.invitation_id);
    channels.add(channel);
    const requestId = crypto.randomUUID();
    const advertisement = host.membership.advertisement();
    const signed = host.membership.proveSpawn(artifact.claim, artifact.secret);
    const proof = { invitation_id: artifact.claim.invitation_id, body_id: artifact.claim.body_id,
      host_id: signed.hostId, boot_id: signed.bootId, nonce: artifact.claim.nonce, signature: signed.signature };
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => finish(reject, new Error("No inviting Workspace answered. Remote internet rendezvous is unsupported.")), 5000);
      const finish = (settle, value) => { clearTimeout(timeout); channel.close(); channels.delete(channel); settle(value); };
      channel.addEventListener("message", ({ data }) => {
        if (data?.request_id !== requestId) return;
        if (data.type === "join-result") finish(resolve, data.durable);
        if (data.type === "join-refusal") finish(reject, new Error(data.message ?? "Body admission was refused"));
      });
      channel.postMessage({ type: "join-request", request_id: requestId, claim: artifact.claim, advertisement, proof });
    });
  }

  render();
  return Object.freeze({ isOpen: () => open, isJoining: () => Boolean(invitation && !session.current()), render, close: () => { for (const channel of channels) channel.close(); channels.clear(); } });
}

function escapeText(value) {
  const span = document.createElement("span"); span.textContent = String(value); return span.innerHTML;
}
