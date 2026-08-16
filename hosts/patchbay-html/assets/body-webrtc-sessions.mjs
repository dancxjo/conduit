import { BodyWebRtcSession } from "./body-webrtc-session.mjs";

const MAXIMUM_WEB_RTC_SESSIONS = 16;

function negotiationIdentity(value) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error("invalid WebRTC negotiation identity");
  }
  return value;
}

/** Finite product-side composition of Body grants, signals, and peer sessions. */
export class BodyWebRtcSessions {
  #wasmBytes;
  #sendSignal;
  #requestGrant;
  #onState;
  #createSession;
  #sessions = new Map();
  #creating = new Map();
  #pendingSignals = new Map();
  #expectedTotal = null;
  #nextGrantIndex = 0;
  #inFlightGrantIndex = null;
  #begun = false;
  #generation = 0;
  #terminal = null;

  constructor({ wasmBytes, sendSignal, requestGrant, onState, createSession = BodyWebRtcSession.create }) {
    if (!(wasmBytes instanceof ArrayBuffer) || typeof sendSignal !== "function" ||
        typeof requestGrant !== "function" || typeof createSession !== "function") {
      throw new Error("invalid Body WebRTC session composition");
    }
    this.#wasmBytes = wasmBytes;
    this.#sendSignal = sendSignal;
    this.#requestGrant = requestGrant;
    this.#onState = onState;
    this.#createSession = createSession;
  }

  begin() {
    if (this.#begun) return;
    this.#terminal = null;
    this.#begun = true;
    this.#requestGrant(0);
  }

  async acceptGrantFrame(frame) {
    if (this.#terminal !== null || !this.#begun) throw new Error("WebRTC grant stage refused");
    const { index, total, grant } = frame ?? {};
    if (!Number.isInteger(index) || !Number.isInteger(total) || index < 0 ||
        index >= MAXIMUM_WEB_RTC_SESSIONS || total < 0 ||
        total > MAXIMUM_WEB_RTC_SESSIONS) {
      throw new Error("invalid WebRTC grant bounds");
    }
    if (this.#expectedTotal === null) this.#expectedTotal = total;
    if (this.#expectedTotal !== total) throw new Error("WebRTC grant total changed");
    if (index !== this.#nextGrantIndex) throw new Error("WebRTC grant index stage refused");
    if (this.#inFlightGrantIndex !== null) throw new Error("WebRTC grant creation already in flight");
    if (total === 0) {
      if (index !== 0 || grant !== null) throw new Error("invalid empty WebRTC grant set");
      if (this.#pendingSignals.size !== 0) {
        this.#pendingSignals.clear();
        throw new Error("WebRTC signal has no granted session");
      }
      this.#onState?.(this.state());
      this.#nextGrantIndex = 1;
      return;
    }
    if (index >= total || grant === null) throw new Error("missing WebRTC grant");
    const negotiationId = negotiationIdentity(grant.negotiation_id);
    if (this.#sessions.has(negotiationId) || this.#creating.has(negotiationId)) {
      throw new Error("duplicate WebRTC grant");
    }
    if (this.#sessions.size + this.#creating.size >= MAXIMUM_WEB_RTC_SESSIONS) {
      throw new Error("WebRTC session capacity exhausted");
    }
    const generation = this.#generation;
    this.#inFlightGrantIndex = index;
    const record = { session: null };
    this.#creating.set(negotiationId, record);
    let creation;
    try {
      creation = this.#createSession({
        wasmBytes: this.#wasmBytes,
        grant,
        sendSignal: (signal) => this.#sendSignal(signal),
        onSession: (session) => {
          if (typeof session?.close !== "function") throw new Error("invalid WebRTC session ownership");
          if (record.session !== null) throw new Error("duplicate WebRTC session ownership");
          record.session = session;
          if (generation !== this.#generation || this.#terminal !== null) session.close();
        },
      });
    } catch (error) {
      if (this.#creating.get(negotiationId) === record) this.#creating.delete(negotiationId);
      this.#inFlightGrantIndex = null;
      throw error;
    }
    let session;
    try {
      session = await creation;
    } catch (error) {
      if (generation === this.#generation && this.#inFlightGrantIndex === index) {
        this.#inFlightGrantIndex = null;
      }
      throw error;
    } finally {
      if (this.#creating.get(negotiationId) === record) {
        this.#creating.delete(negotiationId);
      }
    }
    if (generation === this.#generation && this.#inFlightGrantIndex === index) {
      this.#inFlightGrantIndex = null;
    }
    if (this.#terminal !== null || generation !== this.#generation) {
      session.close();
      throw new Error("stale WebRTC session creation");
    }
    this.#sessions.set(negotiationId, session);
    this.#nextGrantIndex += 1;
    const pending = this.#pendingSignals.get(negotiationId);
    if (pending !== undefined) {
      this.#pendingSignals.delete(negotiationId);
      await session.acceptSignal(pending);
    }
    this.#onState?.(this.state());
    if (index + 1 < total) {
      this.#requestGrant(index + 1);
    } else if (this.#pendingSignals.size !== 0) {
      this.#pendingSignals.clear();
      throw new Error("WebRTC signal negotiation was not granted");
    }
  }

  async acceptSignal(frame) {
    if (this.#terminal !== null || !this.#begun) throw new Error("WebRTC signal stage refused");
    const negotiationId = negotiationIdentity(frame?.signal?.negotiation_id);
    const session = this.#sessions.get(negotiationId);
    if (session !== undefined) {
      await session.acceptSignal(frame);
      this.#onState?.(this.state());
      return;
    }
    if (this.#pendingSignals.has(negotiationId)) throw new Error("duplicate pending WebRTC signal");
    if (this.#pendingSignals.size >= MAXIMUM_WEB_RTC_SESSIONS) {
      throw new Error("pending WebRTC signal capacity exhausted");
    }
    this.#pendingSignals.set(negotiationId, frame);
  }

  reset(reason = "presence-lost") {
    if (!this.#begun && this.#terminal !== null) return;
    this.#generation += 1;
    for (const session of this.#sessions.values()) session.close();
    for (const record of this.#creating.values()) record.session?.close();
    this.#sessions.clear();
    this.#creating.clear();
    this.#pendingSignals.clear();
    this.#expectedTotal = null;
    this.#nextGrantIndex = 0;
    this.#inFlightGrantIndex = null;
    this.#begun = false;
    this.#terminal = reason;
    this.#onState?.(this.state());
  }

  state() {
    return Object.freeze({
      expectedTotal: this.#expectedTotal,
      nextGrantIndex: this.#nextGrantIndex,
      inFlightGrantIndex: this.#inFlightGrantIndex,
      activeSessions: this.#sessions.size,
      creatingSessions: this.#creating.size,
      pendingSignals: this.#pendingSignals.size,
      sessions: Object.freeze([...this.#sessions.values()].map((session) => session.state())),
      terminalReason: this.#terminal,
    });
  }
}
