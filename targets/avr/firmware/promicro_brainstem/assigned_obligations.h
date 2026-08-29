#pragma once

#include <stdint.h>

namespace conduit::promicro {

constexpr uint8_t kAssignedObligationCapacity = 1;
constexpr uint8_t kGroupZeroRequestBytes = 2;
constexpr uint8_t kGroupZeroResponseBytes = 26;
constexpr uint16_t kMaximumObservationDeadlineMs = 2000;

enum class ObligationKind : uint8_t {
  kObserveCreateGroupZero = 1,
};

enum class AdmissionFailure : uint8_t {
  kNone,
  kInvalidIdentity,
  kUnsupportedKind,
  kInvalidBounds,
  kCapacity,
  kDuplicate,
};

enum class TerminalDisposition : uint8_t {
  kPending,
  kCompleted,
  kCancelled,
  kDeadlineExpired,
  kProviderUnavailable,
  kDeviceNoResponse,
  kMalformedResponse,
};

struct AssignedObligation {
  uint32_t plan_fragment_id;
  uint16_t operation_id;
  ObligationKind kind;
  uint8_t request_bytes;
  uint8_t response_bytes;
  uint16_t deadline_ms;
};

class ObligationSlot {
 public:
  AdmissionFailure admit(const AssignedObligation& candidate) {
    if (candidate.plan_fragment_id == 0 || candidate.operation_id == 0) {
      return AdmissionFailure::kInvalidIdentity;
    }
    if (candidate.kind != ObligationKind::kObserveCreateGroupZero) {
      return AdmissionFailure::kUnsupportedKind;
    }
    if (candidate.request_bytes != kGroupZeroRequestBytes ||
        candidate.response_bytes != kGroupZeroResponseBytes ||
        candidate.deadline_ms == 0 ||
        candidate.deadline_ms > kMaximumObservationDeadlineMs) {
      return AdmissionFailure::kInvalidBounds;
    }
    if (occupied_) {
      if (obligation_.plan_fragment_id == candidate.plan_fragment_id &&
          obligation_.operation_id == candidate.operation_id) {
        return AdmissionFailure::kDuplicate;
      }
      return AdmissionFailure::kCapacity;
    }
    obligation_ = candidate;
    disposition_ = TerminalDisposition::kPending;
    occupied_ = true;
    return AdmissionFailure::kNone;
  }

  bool matches(uint32_t plan_fragment_id, uint16_t operation_id) const {
    return occupied_ && obligation_.plan_fragment_id == plan_fragment_id &&
           obligation_.operation_id == operation_id;
  }

  bool finish(uint32_t plan_fragment_id, uint16_t operation_id,
              TerminalDisposition disposition) {
    if (!matches(plan_fragment_id, operation_id) ||
        disposition == TerminalDisposition::kPending) {
      return false;
    }
    disposition_ = disposition;
    return true;
  }

  bool occupied() const { return occupied_; }
  TerminalDisposition disposition() const { return disposition_; }

 private:
  AssignedObligation obligation_{};
  TerminalDisposition disposition_ = TerminalDisposition::kPending;
  bool occupied_ = false;
};

}  // namespace conduit::promicro
