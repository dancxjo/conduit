#include "protocol.h"

#include <assert.h>

using conduit::promicro::CommandBuffer;
using conduit::promicro::Request;

static Request request(CommandBuffer& buffer, const char* bytes) {
  Request result = Request::kUnsupported;
  while (*bytes != '\0') {
    result = buffer.push(*bytes++);
  }
  return result;
}

int main() {
  CommandBuffer buffer;
  assert(request(buffer, "HELLO\n") == Request::kHello);
  assert(request(buffer, "STATUS\n") == Request::kStatus);
  assert(request(buffer, "hello\n") == Request::kUnsupported);
  assert(request(buffer, "STATUS trailing\n") == Request::kUnsupported);

  for (size_t i = 0; i < conduit::promicro::kCommandCapacity + 1; ++i) {
    buffer.push('x');
  }
  assert(buffer.push('\n') == Request::kOverflow);
  assert(request(buffer, "HELLO\n") == Request::kHello);
}
