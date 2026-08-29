#pragma once

#include <stddef.h>
#include <stdint.h>

namespace conduit::promicro::create_oi {

constexpr uint32_t kBaud = 57600;
constexpr int16_t kMaximumWheelSpeedMmS = 500;
constexpr size_t kMaximumCommandBytes = 5;
constexpr uint8_t kPlayLed = 1U << 1U;
constexpr uint8_t kAdvanceLed = 1U << 3U;
constexpr uint8_t kLedMask = kPlayLed | kAdvanceLed;

enum class EncodeFailure : uint8_t {
  kNone,
  kUnsupportedPacket,
  kWheelSpeedOutOfRange,
};

struct EncodedCommand {
  uint8_t bytes[kMaximumCommandBytes];
  uint8_t length;
  EncodeFailure failure;

  bool valid() const { return failure == EncodeFailure::kNone; }
};

inline EncodedCommand command(uint8_t first) {
  return {{first, 0, 0, 0, 0}, 1, EncodeFailure::kNone};
}

inline EncodedCommand start() { return command(128); }
inline EncodedCommand safe() { return command(131); }
inline EncodedCommand full() { return command(132); }
inline EncodedCommand seek_dock() { return command(143); }

inline bool supported_sensor_packet(uint8_t packet) {
  return packet <= 1 || (packet >= 7 && packet <= 38 && packet != 33);
}

inline EncodedCommand query_sensor(uint8_t packet) {
  if (!supported_sensor_packet(packet)) {
    return {{0, 0, 0, 0, 0}, 0, EncodeFailure::kUnsupportedPacket};
  }
  return {{142, packet, 0, 0, 0}, 2, EncodeFailure::kNone};
}

inline EncodedCommand stream_sensor(uint8_t packet) {
  if (!supported_sensor_packet(packet)) {
    return {{0, 0, 0, 0, 0}, 0, EncodeFailure::kUnsupportedPacket};
  }
  return {{148, 1, packet, 0, 0}, 3, EncodeFailure::kNone};
}

inline EncodedCommand pause_stream() {
  return {{150, 0, 0, 0, 0}, 2, EncodeFailure::kNone};
}

inline EncodedCommand lights(uint8_t mask, uint8_t color, uint8_t intensity) {
  return {{139, static_cast<uint8_t>(mask & kLedMask), color, intensity, 0},
          4, EncodeFailure::kNone};
}

inline bool wheel_speed_valid(int16_t speed) {
  const int32_t widened = speed;
  return widened >= -kMaximumWheelSpeedMmS &&
         widened <= kMaximumWheelSpeedMmS;
}

inline EncodedCommand drive_direct(int16_t left_mm_s, int16_t right_mm_s) {
  if (!wheel_speed_valid(left_mm_s) || !wheel_speed_valid(right_mm_s)) {
    return {{0, 0, 0, 0, 0}, 0, EncodeFailure::kWheelSpeedOutOfRange};
  }
  const uint16_t left = static_cast<uint16_t>(left_mm_s);
  const uint16_t right = static_cast<uint16_t>(right_mm_s);
  return {{145, static_cast<uint8_t>(right >> 8U),
           static_cast<uint8_t>(right), static_cast<uint8_t>(left >> 8U),
           static_cast<uint8_t>(left)},
          5, EncodeFailure::kNone};
}

inline EncodedCommand stop() { return drive_direct(0, 0); }

}  // namespace conduit::promicro::create_oi
