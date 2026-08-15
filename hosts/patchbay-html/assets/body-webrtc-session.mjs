import { BrowserWebRtcDataChannelLine } from "./webrtc-datachannel-line.mjs";
import {
  ingestWebRtcSession,
  instantiateGrantedWebRtcSession,
  takeWebRtcSessionOutput,
  webRtcSessionLineLimits,
} from "./webrtc-session-runtime.mjs";

const MAXIMUM_DESCRIPTION_BYTES = 4096;
const encoder = new TextEncoder();

function exactText(value, label) {
  if (typeof value !== "string" || value.length === 0 || encoder.encode(value).length > MAXIMUM_DESCRIPTION_BYTES) {
    throw new Error(`invalid ${label}`);
  }
  return value;
}

function exactHello(value) {
  if (!Array.isArray(value) || value.length === 0 || value.length > 1024 ||
      !value.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)) {
    throw new Error("invalid session Hello");
  }
  return Object.freeze([...value]);
}

function gathered(connection) {
  if (connection.iceGatheringState === "complete") return Promise.resolve();
  return new Promise((resolve) => connection.addEventListener("icegatheringstatechange", () => {
    if (connection.iceGatheringState === "complete") resolve();
  }));
}

export class BodyWebRtcSession {
  #grant;
  #sendSignal;
  #runtime;
  #hello;
  #limits;
  #peer;
  #line;
  #lineArrival;
  #ready;
  #resolveReady;
  #rejectReady;
  #signalAccepted = false;
  #sessionReady = false;
  #terminal = null;
  #terminalDetail = null;

  static async create({ wasmBytes, grant, sendSignal }) {
    if (typeof sendSignal !== "function") throw new Error("sendSignal callback is required");
    const role = grant?.role;
    if (role !== "source" && role !== "sink") throw new Error("invalid Body grant role");
    const exactGrant = Object.freeze({
      negotiation_id: exactText(grant.negotiation_id, "negotiation identity"),
      role,
      peer_host_id: exactText(grant.peer_host_id, "peer Host identity"),
      peer_boot_id: exactText(grant.peer_boot_id, "peer Boot identity"),
      session_hello: exactHello(grant.session_hello),
    });
    const runtime = await instantiateGrantedWebRtcSession(wasmBytes, exactGrant);
    const session = new BodyWebRtcSession(exactGrant, sendSignal, runtime);
    if (role === "source") {
      try {
        await session.#offer();
      } catch (error) {
        session.#fail("offer-refused", error);
        throw error;
      }
    }
    return session;
  }

  constructor(grant, sendSignal, runtime) {
    this.#grant = grant;
    this.#sendSignal = sendSignal;
    this.#runtime = runtime;
    this.#limits = webRtcSessionLineLimits(runtime);
    const hello = takeWebRtcSessionOutput(runtime);
    if (hello === null) throw new Error("granted session emitted no Hello");
    this.#hello = Object.freeze([...hello]);
    this.#peer = new RTCPeerConnection({ iceServers: [] });
    this.#ready = new Promise((resolve, reject) => {
      this.#resolveReady = resolve;
      this.#rejectReady = reject;
    });
    void this.#ready.catch(() => {});
    this.#peer.addEventListener("connectionstatechange", () => {
      if (["failed", "closed", "disconnected"].includes(this.#peer.connectionState)) {
        this.#fail(`peer-${this.#peer.connectionState}`);
      }
    });
    if (grant.role === "source") {
      const channel = this.#peer.createDataChannel("conduit-line", { ordered: true });
      this.#installLine(channel);
      this.#lineArrival = Promise.resolve();
    } else {
      this.#lineArrival = new Promise((resolve) => this.#peer.addEventListener(
        "datachannel",
        (event) => {
          this.#installLine(event.channel);
          resolve();
        },
        { once: true },
      ));
    }
  }

  #installLine(channel) {
    this.#line = new BrowserWebRtcDataChannelLine({ channel, ...this.#limits });
    void this.#line.closed().then((terminal) => this.#fail(`line-${terminal.reason}`));
  }

  async #offer() {
    await this.#peer.setLocalDescription(await this.#peer.createOffer());
    await gathered(this.#peer);
    this.#emit("offer");
  }

  #emit(description) {
    const sdp = this.#peer.localDescription?.sdp;
    exactText(sdp, "local SDP");
    this.#sendSignal(Object.freeze({
      targetHostId: this.#grant.peer_host_id,
      targetBootId: this.#grant.peer_boot_id,
      signal: Object.freeze({
        negotiation_id: this.#grant.negotiation_id,
        description,
        session_hello: this.#hello,
        sdp,
      }),
    }));
  }

  async acceptSignal(frame) {
    if (this.#terminal !== null || this.#signalAccepted) throw new Error("signal stage refused");
    const expectedDescription = this.#grant.role === "source" ? "answer" : "offer";
    const signal = frame?.signal;
    if (frame?.source_host_id !== this.#grant.peer_host_id ||
        frame?.source_boot_id !== this.#grant.peer_boot_id ||
        signal?.negotiation_id !== this.#grant.negotiation_id ||
        signal?.description !== expectedDescription ||
        exactText(signal?.sdp, "remote SDP") !== signal.sdp ||
        JSON.stringify(exactHello(signal?.session_hello)) !== JSON.stringify(this.#hello)) {
      throw new Error("signal identity refused");
    }
    this.#signalAccepted = true;
    try {
      await this.#peer.setRemoteDescription({ type: expectedDescription, sdp: signal.sdp });
      if (this.#grant.role === "sink") {
        await this.#peer.setLocalDescription(await this.#peer.createAnswer());
        await gathered(this.#peer);
        this.#emit("answer");
      }
      void this.#activate();
    } catch (error) {
      this.#fail("negotiation-refused", error);
      throw error;
    }
  }

  async #activate() {
    try {
      await this.#lineArrival;
      await this.#line.open();
      let peerHello;
      if (this.#grant.role === "source") {
        const sentHello = this.#line.send(Uint8Array.from(this.#hello));
        if (!sentHello.accepted) throw new Error(`Hello send refused: ${sentHello.reason}`);
        peerHello = await this.#line.receive();
      } else {
        peerHello = await this.#line.receive();
        const sentHello = this.#line.send(Uint8Array.from(this.#hello));
        if (!sentHello.accepted) throw new Error(`Hello send refused: ${sentHello.reason}`);
      }
      if (!peerHello.ok || ingestWebRtcSession(this.#runtime, peerHello.bytes) < 0) {
        throw new Error("peer Hello refused");
      }
      const ready = takeWebRtcSessionOutput(this.#runtime);
      if (ready === null) throw new Error("Ready output missing");
      let peerReady;
      if (this.#grant.role === "source") {
        await this.#line.writable(ready.byteLength);
        if (!this.#line.send(ready).accepted) throw new Error("Ready send refused");
        peerReady = await this.#line.receive();
      } else {
        peerReady = await this.#line.receive();
      }
      if (!peerReady.ok || ingestWebRtcSession(this.#runtime, peerReady.bytes) !== 1) {
        throw new Error("peer Ready refused");
      }
      if (this.#grant.role === "sink") {
        await this.#line.writable(ready.byteLength);
        if (!this.#line.send(ready).accepted) throw new Error("Ready send refused");
      }
      this.#sessionReady = true;
      this.#resolveReady(this.state());
    } catch (error) {
      this.#fail("session-refused", error);
    }
  }

  ready() {
    return this.#ready;
  }

  state() {
    return Object.freeze({
      negotiationId: this.#grant.negotiation_id,
      role: this.#grant.role,
      peerHostId: this.#grant.peer_host_id,
      peerBootId: this.#grant.peer_boot_id,
      peerState: this.#peer.connectionState,
      line: this.#line?.state() ?? null,
      sessionReady: this.#sessionReady,
      terminalReason: this.#terminal,
      terminalDetail: this.#terminalDetail,
    });
  }

  close() {
    if (this.#line === undefined) {
      this.#fail("closed-before-line");
      return;
    }
    this.#line.close();
  }

  #fail(reason, cause) {
    if (this.#terminal !== null) return;
    this.#terminal = reason;
    this.#terminalDetail = cause instanceof Error ? cause.message : null;
    this.#sessionReady = false;
    if (this.#line === undefined || this.#line.state().readyState === "closed") {
      this.#peer.close();
    } else {
      this.#line.close();
      void this.#line.closed().then(() => this.#peer.close());
    }
    this.#rejectReady(cause ?? new Error(reason));
  }
}
