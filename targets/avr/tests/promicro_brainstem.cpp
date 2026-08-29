#include "../firmware/promicro_brainstem/assigned_obligations.h"
#include "../firmware/promicro_brainstem/create_oi.h"
#include "../firmware/promicro_brainstem/protocol.h"

#include <assert.h>

using conduit::promicro::CommandBuffer;
using conduit::promicro::Request;

static void assert_command(
    const conduit::promicro::create_oi::EncodedCommand& command,
    const uint8_t* expected, size_t length) {
  assert(command.valid());
  assert(command.length == length);
  for (size_t i = 0; i < length; ++i) {
    assert(command.bytes[i] == expected[i]);
  }
}

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

  using namespace conduit::promicro::create_oi;
  const uint8_t start_bytes[] = {128};
  const uint8_t safe_bytes[] = {131};
  const uint8_t full_bytes[] = {132};
  const uint8_t query_bytes[] = {142, 0};
  const uint8_t stream_bytes[] = {148, 1, 0};
  const uint8_t pause_bytes[] = {150, 0};
  const uint8_t light_bytes[] = {139, 0x0a, 42, 255};
  const uint8_t dock_bytes[] = {143};
  const uint8_t drive_bytes[] = {145, 0xff, 0x38, 0x00, 0x64};
  const uint8_t stop_bytes[] = {145, 0, 0, 0, 0};
  assert_command(start(), start_bytes, sizeof(start_bytes));
  assert_command(safe(), safe_bytes, sizeof(safe_bytes));
  assert_command(full(), full_bytes, sizeof(full_bytes));
  assert_command(query_sensor(0), query_bytes, sizeof(query_bytes));
  assert_command(stream_sensor(0), stream_bytes, sizeof(stream_bytes));
  assert_command(pause_stream(), pause_bytes, sizeof(pause_bytes));
  assert_command(lights(0xff, 42, 255), light_bytes, sizeof(light_bytes));
  assert_command(seek_dock(), dock_bytes, sizeof(dock_bytes));
  assert_command(drive_direct(100, -200), drive_bytes, sizeof(drive_bytes));
  assert_command(stop(), stop_bytes, sizeof(stop_bytes));
  assert(query_sensor(6).failure == EncodeFailure::kUnsupportedPacket);
  assert(drive_direct(501, 0).failure ==
         EncodeFailure::kWheelSpeedOutOfRange);

  using namespace conduit::promicro;
  ObligationSlot slot;
  const AssignedObligation observation{0x10203040, 7,
      ObligationKind::kObserveCreateGroupZero, 2, 26, 500};
  assert(slot.admit(observation) == AdmissionFailure::kNone);
  assert(slot.admit(observation) == AdmissionFailure::kDuplicate);
  AssignedObligation competing = observation;
  competing.operation_id = 8;
  assert(slot.admit(competing) == AdmissionFailure::kCapacity);
  assert(!slot.finish(0x10203041, 7, TerminalDisposition::kCompleted));
  assert(!slot.finish(0x10203040, 7, TerminalDisposition::kPending));
  assert(slot.finish(0x10203040, 7, TerminalDisposition::kCompleted));
  assert(slot.disposition() == TerminalDisposition::kCompleted);

  ObligationSlot invalid_slot;
  AssignedObligation invalid = observation;
  invalid.plan_fragment_id = 0;
  assert(invalid_slot.admit(invalid) == AdmissionFailure::kInvalidIdentity);
  invalid = observation;
  invalid.deadline_ms = kMaximumObservationDeadlineMs + 1;
  assert(invalid_slot.admit(invalid) == AdmissionFailure::kInvalidBounds);
  invalid = observation;
  invalid.kind = static_cast<ObligationKind>(99);
  assert(invalid_slot.admit(invalid) == AdmissionFailure::kUnsupportedKind);
}
