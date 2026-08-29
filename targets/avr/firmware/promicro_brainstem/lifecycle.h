#pragma once

#include <stdint.h>

namespace conduit::promicro {

struct HostId {
  uint32_t value;
};

struct BootId {
  uint32_t value;
};

struct PlanFragmentId {
  uint32_t value;
};

struct OperationId {
  uint16_t value;
};

struct ActivePlayId {
  uint32_t value;
};

struct AuthorityGrantId {
  uint32_t value;
};

struct BootBinding {
  HostId host_id;
  BootId boot_id;
  uint32_t offer_generation;
};

struct ObservationActivation {
  HostId host_id;
  BootId boot_id;
  uint32_t offer_generation;
  PlanFragmentId plan_fragment_id;
  OperationId operation_id;
  ActivePlayId active_play_id;
  AuthorityGrantId authority_grant_id;
};

enum class BootBindResult : uint8_t {
  kBound,
  kAlreadyBound,
  kInvalidIdentity,
  kConflictingBinding,
};

enum class ActivationResult : uint8_t {
  kAdmitted,
  kAlreadyAdmitted,
  kBootAbsent,
  kInvalidIdentity,
  kStaleHost,
  kStaleBoot,
  kStaleOfferGeneration,
  kConflictingActivation,
};

class BrainstemLifecycle {
 public:
  BootBindResult bind_boot(const BootBinding& binding) {
    if (!valid(binding)) {
      return BootBindResult::kInvalidIdentity;
    }
    if (!boot_bound_) {
      binding_ = binding;
      boot_bound_ = true;
      return BootBindResult::kBound;
    }
    if (same(binding_, binding)) {
      return BootBindResult::kAlreadyBound;
    }
    return BootBindResult::kConflictingBinding;
  }

  ActivationResult admit(const ObservationActivation& activation) {
    if (!boot_bound_) {
      return ActivationResult::kBootAbsent;
    }
    if (!valid(activation)) {
      return ActivationResult::kInvalidIdentity;
    }
    if (activation.host_id.value != binding_.host_id.value) {
      return ActivationResult::kStaleHost;
    }
    if (activation.boot_id.value != binding_.boot_id.value) {
      return ActivationResult::kStaleBoot;
    }
    if (activation.offer_generation != binding_.offer_generation) {
      return ActivationResult::kStaleOfferGeneration;
    }
    if (!activation_admitted_) {
      activation_ = activation;
      activation_admitted_ = true;
      return ActivationResult::kAdmitted;
    }
    if (same(activation_, activation)) {
      return ActivationResult::kAlreadyAdmitted;
    }
    return ActivationResult::kConflictingActivation;
  }

  bool boot_bound() const { return boot_bound_; }
  bool activation_admitted() const { return activation_admitted_; }
  bool matches_activation(uint32_t plan_fragment_id,
                          uint16_t operation_id) const {
    return activation_admitted_ &&
           activation_.plan_fragment_id.value == plan_fragment_id &&
           activation_.operation_id.value == operation_id;
  }

 private:
  static bool valid(const BootBinding& binding) {
    return binding.host_id.value != 0 && binding.boot_id.value != 0 &&
           binding.offer_generation != 0;
  }

  static bool valid(const ObservationActivation& activation) {
    return activation.host_id.value != 0 && activation.boot_id.value != 0 &&
           activation.offer_generation != 0 &&
           activation.plan_fragment_id.value != 0 &&
           activation.operation_id.value != 0 &&
           activation.active_play_id.value != 0 &&
           activation.authority_grant_id.value != 0;
  }

  static bool same(const BootBinding& left, const BootBinding& right) {
    return left.host_id.value == right.host_id.value &&
           left.boot_id.value == right.boot_id.value &&
           left.offer_generation == right.offer_generation;
  }

  static bool same(const ObservationActivation& left,
                   const ObservationActivation& right) {
    return left.host_id.value == right.host_id.value &&
           left.boot_id.value == right.boot_id.value &&
           left.offer_generation == right.offer_generation &&
           left.plan_fragment_id.value == right.plan_fragment_id.value &&
           left.operation_id.value == right.operation_id.value &&
           left.active_play_id.value == right.active_play_id.value &&
           left.authority_grant_id.value == right.authority_grant_id.value;
  }

  BootBinding binding_{};
  ObservationActivation activation_{};
  bool boot_bound_ = false;
  bool activation_admitted_ = false;
};

}  // namespace conduit::promicro
