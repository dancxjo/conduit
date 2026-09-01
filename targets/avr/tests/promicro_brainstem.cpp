#include "../firmware/promicro_brainstem/protocol.h"
#include "../firmware/promicro_brainstem/embedded_spore.h"

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
  assert(request(buffer, "ATTEST\n") == Request::kAttest);
  assert(request(buffer, "OFFER\n") == Request::kUnsupported);
  assert(request(buffer, "B 0000000B:00000016:00000001\n") ==
         Request::kUnsupported);
  assert(request(buffer, "A fake-plan-identities\n") ==
         Request::kUnsupported);
  assert(request(buffer, "O fake-operation\n") == Request::kUnsupported);
  for (size_t i = 0; i < conduit::promicro::kCommandCapacity + 1; ++i) {
    buffer.push('x');
  }
  assert(buffer.push('\n') == Request::kOverflow);
  assert(request(buffer, "HELLO\n") == Request::kHello);

  uint8_t spore[conduit::promicro::kSporeRegionBytes];
  memset(spore, 0xff, sizeof(spore));
  memcpy(spore, "CONDUIT_SPORE@1", 16);
  spore[16] = 1;
  uint16_t cursor = conduit::promicro::kSporeFixedBytes;
  const char* fields[] = {"spore/one", "image/one", "invitation/one", "body/one"};
  for (const char* field : fields) {
    const uint8_t length = static_cast<uint8_t>(strlen(field));
    spore[cursor++] = length;
    memcpy(spore + cursor, field, length);
    cursor += length;
  }
  spore[17] = cursor & 0xff;
  spore[18] = cursor >> 8;
  const auto read = [&spore](uint16_t offset) { return spore[offset]; };
  assert(conduit::promicro::embedded_spore_valid(read));
  conduit::promicro::EmbeddedSporeField body{};
  assert(conduit::promicro::embedded_spore_field(read, 3, &body));
  assert(body.length == strlen("body/one"));
  assert(memcmp(spore + body.offset, "body/one", body.length) == 0);
  spore[0] = 'X';
  assert(!conduit::promicro::embedded_spore_valid(read));
}
