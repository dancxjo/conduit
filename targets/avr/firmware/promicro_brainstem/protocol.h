#pragma once

#include <stddef.h>
#include <stdint.h>
#include <string.h>

namespace conduit::promicro {

constexpr size_t kCommandCapacity = 64;

// A bounded maintenance parser, not a Host lifecycle protocol.
enum class Request : uint8_t {
  kHello,
  kStatus,
  kAttest,
  kUnsupported,
  kOverflow,
};

class CommandBuffer {
 public:
  Request push(char byte) {
    if (byte == '\r') {
      return Request::kUnsupported;
    }
    if (byte != '\n') {
      if (!overflowed_ && length_ < kCommandCapacity) {
        bytes_[length_++] = byte;
      } else {
        overflowed_ = true;
      }
      return Request::kUnsupported;
    }
    if (overflowed_) {
      reset();
      return Request::kOverflow;
    }
    const Request result = exact("HELLO")    ? Request::kHello
                           : exact("STATUS") ? Request::kStatus
                           : exact("ATTEST") ? Request::kAttest
                                              : Request::kUnsupported;
    reset();
    return result;
  }

 private:
  bool exact(const char* expected) const {
    const size_t expected_length = strlen(expected);
    return length_ == expected_length &&
           memcmp(bytes_, expected, expected_length) == 0;
  }
  void reset() {
    length_ = 0;
    overflowed_ = false;
  }

  char bytes_[kCommandCapacity]{};
  size_t length_ = 0;
  bool overflowed_ = false;
};

}  // namespace conduit::promicro
