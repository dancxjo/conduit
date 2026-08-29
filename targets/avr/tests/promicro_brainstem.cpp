#include "../firmware/promicro_brainstem/assigned_obligations.h"
#include "../firmware/promicro_brainstem/create_oi.h"
#include "../firmware/promicro_brainstem/create_hil.h"
#include "../firmware/promicro_brainstem/group_zero.h"
#include "../firmware/promicro_brainstem/lifecycle.h"
#include "../firmware/promicro_brainstem/protocol.h"

#include <assert.h>

using conduit::promicro::CommandBuffer;
using conduit::promicro::Request;
using conduit::promicro::ActivationResult;
using conduit::promicro::BootBinding;
using conduit::promicro::BootBindResult;
using conduit::promicro::BrainstemLifecycle;
using conduit::promicro::ObservationActivation;

class FakeUart {
 public:
  bool begin(uint32_t baud) {
    began = true;
    observed_baud = baud;
    return provider_available;
  }
  bool write(const uint8_t* bytes, size_t length) {
    if (!provider_available || transmitted_length + length > sizeof(transmitted)) {
      return false;
    }
    for (size_t index = 0; index < length; ++index) {
      transmitted[transmitted_length++] = bytes[index];
    }
    return true;
  }
  bool available() const { return received_index < received_length; }
  uint8_t read() { return received[received_index++]; }
  void end() {
    ended = true;
    began = false;
  }

  uint8_t transmitted[8]{};
  uint8_t received[conduit::promicro::kGroupZeroBytes]{};
  size_t transmitted_length = 0;
  size_t received_length = 0;
  size_t received_index = 0;
  uint32_t observed_baud = 0;
  bool provider_available = true;
  bool began = false;
  bool ended = false;
};

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
  assert(request(buffer, "B 0000000B:00000016:00000001\n") ==
         Request::kBindBoot);
  assert(buffer.boot_binding().host_id.value == 11);
  assert(buffer.boot_binding().boot_id.value == 22);
  assert(buffer.boot_binding().offer_generation == 1);
  assert(request(buffer,
                 "A 0000000B:00000016:00000001:00000021:002C:00000037:"
                 "00000042\n") == Request::kActivateObservation);
  assert(buffer.activation().host_id.value == 11);
  assert(buffer.activation().boot_id.value == 22);
  assert(buffer.activation().offer_generation == 1);
  assert(buffer.activation().plan_fragment_id.value == 33);
  assert(buffer.activation().operation_id.value == 44);
  assert(buffer.activation().active_play_id.value == 55);
  assert(buffer.activation().authority_grant_id.value == 66);
  assert(request(buffer, "O 00000021:002C:01F4\n") ==
         Request::kExecuteObservation);
  assert(buffer.execution().plan_fragment_id == 33);
  assert(buffer.execution().operation_id == 44);
  assert(buffer.execution().deadline_ms == 500);
  assert(buffer.execution().request_bytes ==
         conduit::promicro::kGroupZeroRequestBytes);
  assert(buffer.execution().response_bytes ==
         conduit::promicro::kGroupZeroResponseBytes);
  assert(request(buffer, "O 00000021:002C:01f4\n") == Request::kMalformed);
  assert(request(buffer, "B 0000000b:00000016:00000001\n") ==
         Request::kMalformed);
  assert(request(buffer, "B 0000000B-00000016:00000001\n") ==
         Request::kMalformed);
  assert(request(buffer, "B 0000000B:00000016:00000001\r\n") ==
         Request::kMalformed);
  assert(request(buffer, "BIND 0000000B:00000016:00000001\n") ==
         Request::kUnsupported);

  for (size_t i = 0; i < conduit::promicro::kCommandCapacity + 1; ++i) {
    buffer.push('x');
  }
  assert(buffer.push('\n') == Request::kOverflow);
  assert(request(buffer, "HELLO\n") == Request::kHello);

  BrainstemLifecycle lifecycle;
  const BootBinding boot{{11}, {22}, 1};
  assert(!lifecycle.boot_bound());
  const ObservationActivation activation{
      {11}, {22}, 1, {33}, {44}, {55}, {66}};
  assert(lifecycle.admit(activation) == ActivationResult::kBootAbsent);
  BootBinding invalid_boot = boot;
  invalid_boot.boot_id.value = 0;
  assert(lifecycle.bind_boot(invalid_boot) == BootBindResult::kInvalidIdentity);
  assert(lifecycle.bind_boot(boot) == BootBindResult::kBound);
  assert(lifecycle.boot_bound());
  assert(lifecycle.bind_boot(boot) == BootBindResult::kAlreadyBound);
  BootBinding conflicting_boot = boot;
  conflicting_boot.boot_id.value = 23;
  assert(lifecycle.bind_boot(conflicting_boot) ==
         BootBindResult::kConflictingBinding);

  ObservationActivation stale_host = activation;
  stale_host.host_id.value = 12;
  assert(lifecycle.admit(stale_host) == ActivationResult::kStaleHost);
  ObservationActivation stale_boot = activation;
  stale_boot.boot_id.value = 23;
  assert(lifecycle.admit(stale_boot) == ActivationResult::kStaleBoot);
  ObservationActivation stale_offer = activation;
  stale_offer.offer_generation = 2;
  assert(lifecycle.admit(stale_offer) ==
         ActivationResult::kStaleOfferGeneration);
  ObservationActivation invalid_activation = activation;
  invalid_activation.authority_grant_id.value = 0;
  assert(lifecycle.admit(invalid_activation) ==
         ActivationResult::kInvalidIdentity);
  assert(lifecycle.admit(activation) == ActivationResult::kAdmitted);
  assert(lifecycle.activation_admitted());
  assert(lifecycle.admit(activation) == ActivationResult::kAlreadyAdmitted);
  ObservationActivation conflicting_activation = activation;
  conflicting_activation.active_play_id.value = 56;
  assert(lifecycle.admit(conflicting_activation) ==
         ActivationResult::kConflictingActivation);

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

  uint8_t group_zero[kGroupZeroBytes]{};
  group_zero[0] = 0b00011011;
  group_zero[1] = 1;
  group_zero[2] = 1;
  group_zero[4] = 1;
  group_zero[6] = 1;
  group_zero[7] = 3;
  group_zero[8] = 0x12;
  group_zero[9] = 0x34;
  group_zero[10] = 137;
  group_zero[11] = 0b0101;
  group_zero[12] = 0xff;
  group_zero[13] = 0x88;
  group_zero[14] = 0x00;
  group_zero[15] = 0x1e;
  group_zero[16] = 3;
  group_zero[17] = 0x37;
  group_zero[18] = 0x78;
  group_zero[19] = 0xff;
  group_zero[20] = 0x10;
  group_zero[21] = 31;
  group_zero[22] = 0x04;
  group_zero[23] = 0xb0;
  group_zero[24] = 0x09;
  group_zero[25] = 0x60;

  GroupZeroDecoder decoder;
  for (uint8_t index = 0; index < kGroupZeroBytes - 1; ++index) {
    assert(decoder.push(group_zero[index]) == DecodeOutcome::kNeedMore);
  }
  assert(decoder.push(group_zero[25]) == DecodeOutcome::kValid);
  assert(decoder.push(0) == DecodeOutcome::kClosed);
  const GroupZeroSample& sample = decoder.sample();
  assert(sample.bump_and_wheel_drop == 0b00011011);
  assert(sample.wall);
  assert(sample.cliff_bits == 0b0101);
  assert(sample.virtual_wall);
  assert(sample.wheel_overcurrents == 3);
  assert(sample.dirt_detect == 0x1234);
  assert(sample.infrared == 137);
  assert(sample.buttons == 0b0101);
  assert(sample.distance_delta_mm == -120);
  assert(sample.angle_delta_degrees == 30);
  assert(sample.charging_state == 3);
  assert(sample.millivolts == 14200);
  assert(sample.milliamps == -240);
  assert(sample.temperature_celsius == 31);
  assert(sample.charge_mah == 1200);
  assert(sample.capacity_mah == 2400);

  ObligationSlot evidence_slot;
  assert(evidence_slot.admit(observation) == AdmissionFailure::kNone);
  TerminalEvidence evidence{};
  assert(finish_obligation(evidence_slot, 0x10203041, 7, decoder, evidence) ==
         EvidenceFailure::kStaleIdentity);
  assert(finish_obligation(evidence_slot, 0x10203040, 7, decoder, evidence) ==
         EvidenceFailure::kNone);
  assert(evidence.disposition == TerminalDisposition::kCompleted);
  assert(evidence.response_bytes == 26);
  assert(evidence.payload_valid);

  GroupZeroDecoder absent;
  assert(absent.no_more_bytes() == DecodeOutcome::kDeviceNoResponse);
  assert(absent.no_more_bytes() == DecodeOutcome::kClosed);
  GroupZeroDecoder truncated;
  assert(truncated.push(0) == DecodeOutcome::kNeedMore);
  assert(truncated.no_more_bytes() == DecodeOutcome::kTruncated);
  GroupZeroDecoder cancelled;
  assert(cancelled.cancel() == DecodeOutcome::kCancelled);
  GroupZeroDecoder expired;
  assert(expired.deadline_expired() == DecodeOutcome::kDeadlineExpired);
  GroupZeroDecoder unavailable;
  assert(unavailable.provider_unavailable() ==
         DecodeOutcome::kProviderUnavailable);

  const uint8_t malformed_indices[] = {0, 1, 11, 16};
  for (uint8_t malformed_index : malformed_indices) {
    uint8_t malformed[kGroupZeroBytes];
    for (uint8_t index = 0; index < kGroupZeroBytes; ++index) {
      malformed[index] = group_zero[index];
    }
    malformed[malformed_index] =
        malformed_index == 0 ? 0x20 : malformed_index == 11 ? 0x02 : 6;
    GroupZeroDecoder malformed_decoder;
    for (uint8_t byte : malformed) {
      malformed_decoder.push(byte);
    }
    assert(malformed_decoder.outcome() == DecodeOutcome::kMalformed);
  }

  ObligationSlot hil_slot;
  FakeUart hil_uart;
  for (uint8_t index = 0; index < kGroupZeroBytes; ++index) {
    hil_uart.received[index] = group_zero[index];
  }
  hil_uart.received_length = kGroupZeroBytes;
  CreateGroupZeroExecutor<FakeUart> executor;
  assert(executor.start(lifecycle, hil_slot, 33, 44, 500, 100, hil_uart) ==
         HilStartResult::kStarted);
  assert(hil_uart.observed_baud == kCreateBaud);
  const uint8_t exact_hil_tx[] = {128, 132, 142, 0};
  assert(hil_uart.transmitted_length == sizeof(exact_hil_tx));
  for (size_t index = 0; index < sizeof(exact_hil_tx); ++index) {
    assert(hil_uart.transmitted[index] == exact_hil_tx[index]);
  }
  executor.tick(101, hil_uart);
  assert(!executor.running());
  assert(hil_uart.ended);
  assert(executor.evidence_failure() == EvidenceFailure::kNone);
  assert(executor.evidence().disposition == TerminalDisposition::kCompleted);
  assert(executor.evidence().response_bytes == kGroupZeroBytes);
  assert(executor.start(lifecycle, hil_slot, 33, 44, 500, 102, hil_uart) ==
         HilStartResult::kAlreadyTerminal);

  ObligationSlot stale_hil_slot;
  FakeUart stale_hil_uart;
  CreateGroupZeroExecutor<FakeUart> stale_executor;
  assert(stale_executor.start(lifecycle, stale_hil_slot, 34, 44, 500, 0,
                              stale_hil_uart) ==
         HilStartResult::kStaleActivation);
  assert(!stale_hil_uart.began);

  ObligationSlot deadline_slot;
  FakeUart deadline_uart;
  CreateGroupZeroExecutor<FakeUart> deadline_executor;
  assert(deadline_executor.start(lifecycle, deadline_slot, 33, 44, 20, 1000,
                                 deadline_uart) == HilStartResult::kStarted);
  deadline_executor.tick(1019, deadline_uart);
  assert(deadline_executor.running());
  deadline_executor.tick(1020, deadline_uart);
  assert(!deadline_executor.running());
  assert(deadline_uart.ended);
  assert(deadline_executor.evidence().disposition ==
         TerminalDisposition::kDeadlineExpired);

  ObligationSlot unavailable_slot;
  FakeUart unavailable_uart;
  unavailable_uart.provider_available = false;
  CreateGroupZeroExecutor<FakeUart> unavailable_executor;
  assert(unavailable_executor.start(lifecycle, unavailable_slot, 33, 44, 20,
                                    0, unavailable_uart) ==
         HilStartResult::kProviderUnavailable);
  assert(unavailable_uart.ended);
  assert(unavailable_executor.evidence().disposition ==
         TerminalDisposition::kProviderUnavailable);
}
