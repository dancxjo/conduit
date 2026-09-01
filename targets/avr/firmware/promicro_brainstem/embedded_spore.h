#pragma once

#include <stdint.h>

namespace conduit::promicro {

constexpr uint16_t kSporeRegionStart = 0x6c00;
constexpr uint16_t kSporeRegionBytes = 1024;
constexpr uint8_t kSporeFieldCount = 4;
constexpr uint8_t kSporeIdBytes = 128;
constexpr uint16_t kSporeFixedBytes = 91;

struct EmbeddedSporeField {
  uint16_t offset;
  uint8_t length;
};

template <typename Reader>
bool embedded_spore_field(Reader read, uint8_t wanted,
                          EmbeddedSporeField* selected) {
  constexpr char kMagic[] = "CONDUIT_SPORE@1";
  for (uint8_t index = 0; index < sizeof(kMagic); ++index) {
    if (read(index) != static_cast<uint8_t>(kMagic[index])) return false;
  }
  if (read(16) != 1) return false;
  const uint16_t total =
      static_cast<uint16_t>(read(17)) |
      (static_cast<uint16_t>(read(18)) << 8);
  if (total < kSporeFixedBytes + kSporeFieldCount * 2 ||
      total > kSporeRegionBytes) {
    return false;
  }
  uint16_t cursor = kSporeFixedBytes;
  for (uint8_t field = 0; field < kSporeFieldCount; ++field) {
    const uint8_t length = read(cursor++);
    if (length == 0 || length > kSporeIdBytes || cursor + length > total) {
      return false;
    }
    for (uint8_t index = 0; index < length; ++index) {
      const uint8_t byte = read(cursor + index);
      if (byte < 0x21 || byte > 0x7e) return false;
    }
    if (field == wanted && selected != nullptr) {
      selected->offset = cursor;
      selected->length = length;
    }
    cursor += length;
  }
  return cursor == total;
}

template <typename Reader>
bool embedded_spore_valid(Reader read) {
  return embedded_spore_field(read, 0, nullptr);
}

}  // namespace conduit::promicro
