#pragma once

#include "assigned_obligations.h"
#include "group_zero.h"
#include "lifecycle.h"

#include <stddef.h>
#include <stdint.h>

namespace conduit::promicro {

constexpr uint32_t kCreateBaud = 57600;
constexpr uint8_t kCreateSetupBytes[] = {128, 132};
constexpr uint8_t kCreateGroupZeroRequest[] = {142, 0};
constexpr uint8_t kCreateSetupByteCount = sizeof(kCreateSetupBytes);
constexpr uint8_t kCreateRequestByteCount =
    sizeof(kCreateGroupZeroRequest);
constexpr uint8_t kMaximumRxBytesPerTick = 32;

enum class HilStartResult : uint8_t {
  kStarted,
  kStaleActivation,
  kAdmissionRefused,
  kAlreadyRunning,
  kAlreadyTerminal,
  kProviderUnavailable,
};

template <typename Uart>
class CreateGroupZeroExecutor {
 public:
  HilStartResult start(const BrainstemLifecycle& lifecycle,
                       ObligationSlot& slot, uint32_t plan_fragment_id,
                       uint16_t operation_id, uint16_t deadline_ms,
                       uint32_t now_ms, Uart& uart) {
    if (running_) {
      return HilStartResult::kAlreadyRunning;
    }
    if (terminal()) {
      return HilStartResult::kAlreadyTerminal;
    }
    if (!lifecycle.matches_activation(plan_fragment_id, operation_id)) {
      return HilStartResult::kStaleActivation;
    }
    const AssignedObligation obligation{
        plan_fragment_id, operation_id,
        ObligationKind::kObserveCreateGroupZero, kCreateRequestByteCount,
        kGroupZeroBytes, deadline_ms};
    if (slot.admit(obligation) != AdmissionFailure::kNone) {
      return HilStartResult::kAdmissionRefused;
    }
    slot_ = &slot;
    plan_fragment_id_ = plan_fragment_id;
    operation_id_ = operation_id;
    deadline_ms_ = deadline_ms;
    started_ms_ = now_ms;
    if (!uart.begin(kCreateBaud) ||
        !uart.write(kCreateSetupBytes, kCreateSetupByteCount) ||
        !uart.write(kCreateGroupZeroRequest, kCreateRequestByteCount)) {
      decoder_.provider_unavailable();
      finish(uart);
      return HilStartResult::kProviderUnavailable;
    }
    running_ = true;
    return HilStartResult::kStarted;
  }

  void tick(uint32_t now_ms, Uart& uart) {
    if (!running_) {
      return;
    }
    uint8_t drained = 0;
    while (uart.available() && drained < kMaximumRxBytesPerTick) {
      const DecodeOutcome outcome = decoder_.push(uart.read());
      ++drained;
      if (outcome != DecodeOutcome::kNeedMore) {
        finish(uart);
        return;
      }
    }
    if (static_cast<uint32_t>(now_ms - started_ms_) >= deadline_ms_) {
      decoder_.deadline_expired();
      finish(uart);
    }
  }

  void cancel(Uart& uart) {
    if (!running_) {
      return;
    }
    decoder_.cancel();
    finish(uart);
  }

  void provider_closed(Uart& uart) {
    if (!running_) {
      return;
    }
    decoder_.no_more_bytes();
    finish(uart);
  }

  bool running() const { return running_; }
  bool terminal() const {
    return decoder_.outcome() != DecodeOutcome::kNeedMore;
  }
  const TerminalEvidence& evidence() const { return evidence_; }
  EvidenceFailure evidence_failure() const { return evidence_failure_; }

 private:
  void finish(Uart& uart) {
    uart.end();
    running_ = false;
    if (slot_ != nullptr) {
      evidence_failure_ = finish_obligation(
          *slot_, plan_fragment_id_, operation_id_, decoder_, evidence_);
    }
  }

  GroupZeroDecoder decoder_{};
  TerminalEvidence evidence_{};
  ObligationSlot* slot_ = nullptr;
  uint32_t plan_fragment_id_ = 0;
  uint32_t started_ms_ = 0;
  uint16_t operation_id_ = 0;
  uint16_t deadline_ms_ = 0;
  EvidenceFailure evidence_failure_ = EvidenceFailure::kNonTerminal;
  bool running_ = false;
};

}  // namespace conduit::promicro
