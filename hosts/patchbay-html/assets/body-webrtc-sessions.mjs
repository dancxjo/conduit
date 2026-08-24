import { BodyWebRtcSession } from "./body-webrtc-session.mjs";

const MAXIMUM_WEB_RTC_SESSIONS = 16;
const MAXIMUM_GRANT_GENERATIONS = 2;

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
  #lifecycleGeneration = 0;
  #grantGeneration = 0;
  #retiredNegotiations = new Set();
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
    this.#requestGrant(this.#grantGeneration, 0);
  }

  async acceptGrantFrame(frame) {
    if (this.#terminal !== null || !this.#begun) throw new Error("WebRTC grant stage refused");
    const { generation, index, total, grant } = frame ?? {};
    if (!Number.isInteger(generation) || generation !== this.#grantGeneration) {
      throw new Error("stale WebRTC grant generation");
    }
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
    const lifecycleGeneration = this.#lifecycleGeneration;
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
          if (lifecycleGeneration !== this.#lifecycleGeneration || this.#terminal !== null) session.close();
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
      if (lifecycleGeneration === this.#lifecycleGeneration && this.#inFlightGrantIndex === index) {
        this.#inFlightGrantIndex = null;
      }
      throw error;
    } finally {
      if (this.#creating.get(negotiationId) === record) {
        this.#creating.delete(negotiationId);
      }
    }
    if (lifecycleGeneration === this.#lifecycleGeneration && this.#inFlightGrantIndex === index) {
      this.#inFlightGrantIndex = null;
    }
    if (this.#terminal !== null || lifecycleGeneration !== this.#lifecycleGeneration) {
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
      this.#requestGrant(this.#grantGeneration, index + 1);
    } else if (this.#pendingSignals.size !== 0) {
      this.#pendingSignals.clear();
      throw new Error("WebRTC signal negotiation was not granted");
    }
  }

  async acceptSignal(frame) {
    if (this.#terminal !== null || !this.#begun) throw new Error("WebRTC signal stage refused");
    const negotiationId = negotiationIdentity(frame?.signal?.negotiation_id);
    if (this.#retiredNegotiations.has(negotiationId)) {
      throw new Error("stale WebRTC negotiation identity");
    }
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
    this.#lifecycleGeneration += 1;
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

  replan() {
    if (this.#terminal !== null || !this.#begun || this.#sessions.size === 0 ||
        this.#creating.size !== 0 || this.#pendingSignals.size !== 0 ||
        [...this.#sessions.values()].some((session) => session.state().terminalReason === null)) {
      throw new Error("WebRTC replan requires only terminal current sessions");
    }
    if (this.#grantGeneration + 1 >= MAXIMUM_GRANT_GENERATIONS) {
      throw new Error("WebRTC grant generation capacity exhausted");
    }
    for (const [negotiationId, session] of this.#sessions) {
      this.#retiredNegotiations.add(negotiationId);
      session.close();
    }
    this.#sessions.clear();
    this.#expectedTotal = null;
    this.#nextGrantIndex = 0;
    this.#inFlightGrantIndex = null;
    this.#lifecycleGeneration += 1;
    this.#grantGeneration += 1;
    this.#requestGrant(this.#grantGeneration, 0);
    this.#onState?.(this.state());
    return this.#grantGeneration;
  }

  activatePlan() {
    if (this.#terminal !== null || !this.#begun || this.#expectedTotal !== 0 ||
        this.#sessions.size !== 0 || this.#creating.size !== 0 || this.#pendingSignals.size !== 0 ||
        this.#inFlightGrantIndex !== null || this.#nextGrantIndex !== 1) {
      throw new Error("WebRTC Plan activation requires an exact completed empty grant set");
    }
    if (this.#grantGeneration + 1 >= MAXIMUM_GRANT_GENERATIONS) {
      throw new Error("WebRTC grant generation capacity exhausted");
    }
    this.#expectedTotal = null;
    this.#nextGrantIndex = 0;
    this.#grantGeneration += 1;
    this.#requestGrant(this.#grantGeneration, 0);
    this.#onState?.(this.state());
    return this.#grantGeneration;
  }

  #session(negotiationId) {
    negotiationIdentity(negotiationId);
    if (this.#terminal !== null || !this.#begun) throw new Error("WebRTC sessions are not current");
    const session = this.#sessions.get(negotiationId);
    if (session === undefined) throw new Error("unknown or stale WebRTC negotiation identity");
    return session;
  }

  offerValue(negotiationId, bytes) {
    return this.#session(negotiationId).offerValue(negotiationId, bytes);
  }

  receiveValue(negotiationId) {
    return this.#session(negotiationId).receiveValue(negotiationId);
  }

  pressureNextValue(negotiationId) {
    return this.#session(negotiationId).pressureNextValue(negotiationId);
  }

  deliverValue(negotiationId, sequence) {
    return this.#session(negotiationId).deliverValue(negotiationId, sequence);
  }

  waitDelivered(negotiationId, sequence) {
    return this.#session(negotiationId).waitDelivered(negotiationId, sequence);
  }

  closeLine(negotiationId) {
    this.#session(negotiationId).close();
    this.#onState?.(this.state());
  }

  state() {
    return Object.freeze({
      expectedTotal: this.#expectedTotal,
      generation: this.#grantGeneration,
      nextGrantIndex: this.#nextGrantIndex,
      inFlightGrantIndex: this.#inFlightGrantIndex,
      activeSessions: this.#sessions.size,
      creatingSessions: this.#creating.size,
      pendingSignals: this.#pendingSignals.size,
      retiredNegotiations: this.#retiredNegotiations.size,
      sessions: Object.freeze([...this.#sessions.values()].map((session) => session.state())),
      terminalReason: this.#terminal,
    });
  }
}
