#pragma once

#include <stdint.h>

namespace conduit::promicro {

constexpr uint16_t kRxBoundarySamples = 2048;

struct RxBoundaryEvidence {
  void push(bool high) {
    if (sampled && high != previous_high) {
      ++transitions;
    }
    previous_high = high;
    sampled = true;
    if (high) {
      ++high_samples;
    } else {
      ++low_samples;
    }
  }

  bool stable_high() const {
    return high_samples == kRxBoundarySamples && low_samples == 0 &&
           transitions == 0;
  }

  uint16_t high_samples = 0;
  uint16_t low_samples = 0;
  uint16_t transitions = 0;
  bool previous_high = false;
  bool sampled = false;
};

}  // namespace conduit::promicro
