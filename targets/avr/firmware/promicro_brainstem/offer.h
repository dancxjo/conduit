#pragma once

#include "assigned_obligations.h"
#include "lifecycle.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

namespace conduit::promicro {

enum class ImageProfile : uint8_t {
  kIsolated,
  kCreateHil,
};

struct BuildAttestation {
  const char* build_id;
  const char* source_sha;
  const char* source_digest_sha256;
  ImageProfile profile;
};

enum class OfferResult : uint8_t {
  kOffered,
  kNoExecutableImplementation,
  kBootAbsent,
  kInvalidBuildIdentity,
};

struct CreateObservationOffer {
  BootBinding placement;
  const char* artifact_build;
  uint8_t operation_capacity;
  uint8_t response_byte_capacity;
  uint16_t maximum_deadline_ms;
};

inline bool exact_lower_hex(const char* value, size_t length) {
  if (value == nullptr || strlen(value) != length) {
    return false;
  }
  for (size_t index = 0; index < length; ++index) {
    const char byte = value[index];
    if (!((byte >= '0' && byte <= '9') || (byte >= 'a' && byte <= 'f'))) {
      return false;
    }
  }
  return true;
}

inline bool valid(const BuildAttestation& attestation) {
  return exact_lower_hex(attestation.build_id, 64) &&
         exact_lower_hex(attestation.source_sha, 40) &&
         exact_lower_hex(attestation.source_digest_sha256, 64) &&
         (attestation.profile == ImageProfile::kIsolated ||
          attestation.profile == ImageProfile::kCreateHil);
}

inline OfferResult current_offer(const BrainstemLifecycle& lifecycle,
                                 const BuildAttestation& attestation,
                                 CreateObservationOffer& offer) {
  if (!valid(attestation)) {
    return OfferResult::kInvalidBuildIdentity;
  }
  const BootBinding* placement = lifecycle.binding();
  if (placement == nullptr) {
    return OfferResult::kBootAbsent;
  }
  if (attestation.profile != ImageProfile::kCreateHil) {
    return OfferResult::kNoExecutableImplementation;
  }
  offer = {*placement, attestation.build_id, 1, kGroupZeroResponseBytes,
           kMaximumObservationDeadlineMs};
  return OfferResult::kOffered;
}

}  // namespace conduit::promicro
