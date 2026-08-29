#pragma once

#include <stddef.h>
#include <stdint.h>

#include "assigned_obligations.h"

namespace conduit::promicro {

constexpr uint8_t kGroupZeroBytes = 26;

enum class DecodeOutcome : uint8_t {
  kNeedMore,
  kValid,
  kMalformed,
  kDeviceNoResponse,
  kTruncated,
  kCancelled,
  kDeadlineExpired,
  kProviderUnavailable,
  kClosed,
};

struct GroupZeroSample {
  uint8_t bump_and_wheel_drop;
  bool wall;
  uint8_t cliff_bits;
  bool virtual_wall;
  uint8_t wheel_overcurrents;
  uint16_t dirt_detect;
  uint8_t infrared;
  uint8_t buttons;
  int16_t distance_delta_mm;
  int16_t angle_delta_degrees;
  uint8_t charging_state;
  uint16_t millivolts;
  int16_t milliamps;
  int8_t temperature_celsius;
  uint16_t charge_mah;
  uint16_t capacity_mah;
};

inline uint16_t unsigned_be(const uint8_t* bytes) {
  return static_cast<uint16_t>(static_cast<uint16_t>(bytes[0]) << 8U) |
         bytes[1];
}

inline int16_t signed_be(const uint8_t* bytes) {
  return static_cast<int16_t>(unsigned_be(bytes));
}

class GroupZeroDecoder {
 public:
  DecodeOutcome push(uint8_t byte) {
    if (outcome_ != DecodeOutcome::kNeedMore) {
      return DecodeOutcome::kClosed;
    }
    bytes_[received_++] = byte;
    if (received_ != kGroupZeroBytes) {
      return DecodeOutcome::kNeedMore;
    }
    outcome_ = valid_payload() ? DecodeOutcome::kValid
                               : DecodeOutcome::kMalformed;
    if (outcome_ == DecodeOutcome::kValid) {
      decode();
    }
    return outcome_;
  }

  DecodeOutcome no_more_bytes() {
    if (outcome_ != DecodeOutcome::kNeedMore) {
      return DecodeOutcome::kClosed;
    }
    outcome_ = received_ == 0 ? DecodeOutcome::kDeviceNoResponse
                              : DecodeOutcome::kTruncated;
    return outcome_;
  }

  DecodeOutcome cancel() { return close(DecodeOutcome::kCancelled); }
  DecodeOutcome deadline_expired() {
    return close(DecodeOutcome::kDeadlineExpired);
  }
  DecodeOutcome provider_unavailable() {
    return close(DecodeOutcome::kProviderUnavailable);
  }

  DecodeOutcome outcome() const { return outcome_; }
  uint8_t received() const { return received_; }
  const GroupZeroSample& sample() const { return sample_; }

 private:
  DecodeOutcome close(DecodeOutcome outcome) {
    if (outcome_ != DecodeOutcome::kNeedMore) {
      return DecodeOutcome::kClosed;
    }
    outcome_ = outcome;
    return outcome_;
  }

  bool valid_payload() const {
    if ((bytes_[0] & static_cast<uint8_t>(~0x1fU)) != 0 ||
        (bytes_[11] & static_cast<uint8_t>(~0x05U)) != 0 ||
        bytes_[16] > 5) {
      return false;
    }
    for (uint8_t index = 1; index <= 6; ++index) {
      if (bytes_[index] > 1) {
        return false;
      }
    }
    return true;
  }

  void decode() {
    sample_.bump_and_wheel_drop = bytes_[0];
    sample_.wall = bytes_[1] != 0;
    sample_.cliff_bits = static_cast<uint8_t>(
        (bytes_[2] != 0 ? 1U : 0U) | (bytes_[3] != 0 ? 2U : 0U) |
        (bytes_[4] != 0 ? 4U : 0U) | (bytes_[5] != 0 ? 8U : 0U));
    sample_.virtual_wall = bytes_[6] != 0;
    sample_.wheel_overcurrents = bytes_[7];
    sample_.dirt_detect = unsigned_be(&bytes_[8]);
    sample_.infrared = bytes_[10];
    sample_.buttons = bytes_[11];
    sample_.distance_delta_mm = signed_be(&bytes_[12]);
    sample_.angle_delta_degrees = signed_be(&bytes_[14]);
    sample_.charging_state = bytes_[16];
    sample_.millivolts = unsigned_be(&bytes_[17]);
    sample_.milliamps = signed_be(&bytes_[19]);
    sample_.temperature_celsius = static_cast<int8_t>(bytes_[21]);
    sample_.charge_mah = unsigned_be(&bytes_[22]);
    sample_.capacity_mah = unsigned_be(&bytes_[24]);
  }

  uint8_t bytes_[kGroupZeroBytes]{};
  GroupZeroSample sample_{};
  uint8_t received_ = 0;
  DecodeOutcome outcome_ = DecodeOutcome::kNeedMore;
};

struct TerminalEvidence {
  uint32_t plan_fragment_id;
  uint16_t operation_id;
  TerminalDisposition disposition;
  uint8_t response_bytes;
  bool payload_valid;
};

enum class EvidenceFailure : uint8_t {
  kNone,
  kStaleIdentity,
  kNonTerminal,
  kInvalidSuccess,
};

inline TerminalDisposition disposition_for(DecodeOutcome outcome) {
  switch (outcome) {
    case DecodeOutcome::kValid:
      return TerminalDisposition::kCompleted;
    case DecodeOutcome::kCancelled:
      return TerminalDisposition::kCancelled;
    case DecodeOutcome::kDeadlineExpired:
      return TerminalDisposition::kDeadlineExpired;
    case DecodeOutcome::kProviderUnavailable:
      return TerminalDisposition::kProviderUnavailable;
    case DecodeOutcome::kDeviceNoResponse:
      return TerminalDisposition::kDeviceNoResponse;
    case DecodeOutcome::kMalformed:
    case DecodeOutcome::kTruncated:
    case DecodeOutcome::kClosed:
      return TerminalDisposition::kMalformedResponse;
    case DecodeOutcome::kNeedMore:
      return TerminalDisposition::kPending;
  }
  return TerminalDisposition::kMalformedResponse;
}

inline EvidenceFailure finish_obligation(
    ObligationSlot& slot, uint32_t plan_fragment_id, uint16_t operation_id,
    const GroupZeroDecoder& decoder, TerminalEvidence& evidence) {
  if (!slot.matches(plan_fragment_id, operation_id)) {
    return EvidenceFailure::kStaleIdentity;
  }
  const TerminalDisposition disposition = disposition_for(decoder.outcome());
  if (disposition == TerminalDisposition::kPending) {
    return EvidenceFailure::kNonTerminal;
  }
  const bool valid = decoder.outcome() == DecodeOutcome::kValid;
  if (valid && decoder.received() != kGroupZeroBytes) {
    return EvidenceFailure::kInvalidSuccess;
  }
  if (!slot.finish(plan_fragment_id, operation_id, disposition)) {
    return EvidenceFailure::kStaleIdentity;
  }
  evidence = {plan_fragment_id, operation_id, disposition, decoder.received(),
              valid};
  return EvidenceFailure::kNone;
}

}  // namespace conduit::promicro
