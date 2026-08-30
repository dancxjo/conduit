#pragma once

#include "assigned_obligations.h"
#include "lifecycle.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

namespace conduit::promicro {

constexpr size_t kCommandCapacity = 64;

enum class Request : uint8_t {
  kHello,
  kStatus,
  kAttest,
  kOffer,
  kRxBoundary,
  kBindBoot,
  kActivateObservation,
  kExecuteObservation,
  kMalformed,
  kUnsupported,
  kOverflow,
};

class CommandBuffer {
 public:
  Request push(char byte) {
    if (byte == '\r') {
      malformed_ = true;
      return Request::kUnsupported;
    }
    if (byte != '\n') {
      if (overflowed_) {
        return Request::kUnsupported;
      }
      if (length_ == kCommandCapacity) {
        overflowed_ = true;
        return Request::kUnsupported;
      }
      bytes_[length_++] = byte;
      return Request::kUnsupported;
    }

    Request request = Request::kUnsupported;
    if (overflowed_) {
      request = Request::kOverflow;
    } else if (malformed_) {
      request = Request::kMalformed;
    } else if (exact("HELLO")) {
      request = Request::kHello;
    } else if (exact("STATUS")) {
      request = Request::kStatus;
    } else if (exact("ATTEST")) {
      request = Request::kAttest;
    } else if (exact("OFFER")) {
      request = Request::kOffer;
    } else if (exact("RXDIAG")) {
      request = Request::kRxBoundary;
    } else if (starts_with("B ")) {
      request = parse_boot() ? Request::kBindBoot : Request::kMalformed;
    } else if (starts_with("A ")) {
      request = parse_activation() ? Request::kActivateObservation
                                   : Request::kMalformed;
    } else if (starts_with("O ")) {
      request = parse_execution() ? Request::kExecuteObservation
                                  : Request::kMalformed;
    }
    reset();
    return request;
  }

  const BootBinding& boot_binding() const { return boot_binding_; }
  const ObservationActivation& activation() const { return activation_; }
  const AssignedObligation& execution() const { return execution_; }

 private:
  static bool hex_nibble(char byte, uint8_t& value) {
    if (byte >= '0' && byte <= '9') {
      value = static_cast<uint8_t>(byte - '0');
      return true;
    }
    if (byte >= 'A' && byte <= 'F') {
      value = static_cast<uint8_t>(byte - 'A' + 10);
      return true;
    }
    return false;
  }

  bool parse_hex(size_t offset, size_t digits, uint32_t& value) const {
    value = 0;
    for (size_t index = 0; index < digits; ++index) {
      uint8_t nibble = 0;
      if (!hex_nibble(bytes_[offset + index], nibble)) {
        return false;
      }
      value = static_cast<uint32_t>((value << 4) | nibble);
    }
    return true;
  }

  bool parse_boot() {
    if (length_ != 28 || bytes_[10] != ':' || bytes_[19] != ':') {
      return false;
    }
    return parse_hex(2, 8, boot_binding_.host_id.value) &&
           parse_hex(11, 8, boot_binding_.boot_id.value) &&
           parse_hex(20, 8, boot_binding_.offer_generation);
  }

  bool parse_activation() {
    if (length_ != 60 || bytes_[10] != ':' || bytes_[19] != ':' ||
        bytes_[28] != ':' || bytes_[37] != ':' || bytes_[42] != ':' ||
        bytes_[51] != ':') {
      return false;
    }
    uint32_t operation_id = 0;
    const bool parsed =
        parse_hex(2, 8, activation_.host_id.value) &&
        parse_hex(11, 8, activation_.boot_id.value) &&
        parse_hex(20, 8, activation_.offer_generation) &&
        parse_hex(29, 8, activation_.plan_fragment_id.value) &&
        parse_hex(38, 4, operation_id) &&
        parse_hex(43, 8, activation_.active_play_id.value) &&
        parse_hex(52, 8, activation_.authority_grant_id.value);
    activation_.operation_id.value = static_cast<uint16_t>(operation_id);
    return parsed;
  }

  bool parse_execution() {
    if (length_ != 20 || bytes_[10] != ':' || bytes_[15] != ':') {
      return false;
    }
    uint32_t operation_id = 0;
    uint32_t deadline_ms = 0;
    const bool parsed =
        parse_hex(2, 8, execution_.plan_fragment_id) &&
        parse_hex(11, 4, operation_id) && parse_hex(16, 4, deadline_ms);
    execution_.operation_id = static_cast<uint16_t>(operation_id);
    execution_.kind = ObligationKind::kObserveCreateGroupZero;
    execution_.request_bytes = kGroupZeroRequestBytes;
    execution_.response_bytes = kGroupZeroResponseBytes;
    execution_.deadline_ms = static_cast<uint16_t>(deadline_ms);
    return parsed;
  }

  bool exact(const char* expected) const {
    const size_t expected_length = strlen(expected);
    return length_ == expected_length &&
           memcmp(bytes_, expected, expected_length) == 0;
  }

  bool starts_with(const char* expected) const {
    const size_t expected_length = strlen(expected);
    return length_ >= expected_length &&
           memcmp(bytes_, expected, expected_length) == 0;
  }

  void reset() {
    length_ = 0;
    malformed_ = false;
    overflowed_ = false;
  }

  char bytes_[kCommandCapacity]{};
  size_t length_ = 0;
  BootBinding boot_binding_{};
  ObservationActivation activation_{};
  AssignedObligation execution_{};
  bool malformed_ = false;
  bool overflowed_ = false;
};

}  // namespace conduit::promicro
