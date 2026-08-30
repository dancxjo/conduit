#include "assigned_obligations.h"
#include "create_attachment.h"
#include "create_oi.h"
#include "create_hil.h"
#include "group_zero.h"
#include "lifecycle.h"
#include "offer.h"
#include "protocol.h"
#include "rx_boundary.h"

using conduit::promicro::CommandBuffer;
using conduit::promicro::Request;

#if !defined(CONDUIT_AVR_BUILD_ID) || !defined(CONDUIT_AVR_SOURCE_SHA) || \
    !defined(CONDUIT_AVR_SOURCE_DIGEST)
#error "AVR build identity must be supplied by cargo xtask"
#endif

#define WRITE_TEXT(text) Serial.print(F(text))

namespace {

CommandBuffer command;
conduit::promicro::BrainstemLifecycle lifecycle;

#if defined(CONDUIT_CREATE_HIL)
constexpr conduit::promicro::ImageProfile kImageProfile =
    conduit::promicro::ImageProfile::kCreateHil;
#else
constexpr conduit::promicro::ImageProfile kImageProfile =
    conduit::promicro::ImageProfile::kIsolated;
#endif

const conduit::promicro::BuildAttestation kBuildAttestation{
    CONDUIT_AVR_BUILD_ID, CONDUIT_AVR_SOURCE_SHA, CONDUIT_AVR_SOURCE_DIGEST,
    kImageProfile};

void isolate_create_uart() {
  Serial1.end();
  pinMode(conduit::promicro::kCreateRxPin, INPUT);
  pinMode(conduit::promicro::kCreateTxPin, INPUT);
}

#if defined(CONDUIT_CREATE_HIL)
class CreateSerial {
 public:
  bool begin(uint32_t baud) {
    Serial1.begin(baud);
    return true;
  }
  bool write(const uint8_t* bytes, size_t length) {
    return Serial1.write(bytes, length) == length;
  }
  bool available() const { return Serial1.available() > 0; }
  uint8_t read() { return static_cast<uint8_t>(Serial1.read()); }
  void end() { isolate_create_uart(); }
};

CreateSerial create_serial;
conduit::promicro::ObligationSlot create_obligation;
conduit::promicro::CreateGroupZeroExecutor<CreateSerial> create_executor;
bool terminal_reported = false;
uint8_t create_tx_bytes = 0;
#endif

void write_hex(uint32_t value, uint8_t digits) {
  constexpr char kHex[] = "0123456789ABCDEF";
  char encoded[8];
  for (uint8_t index = 0; index < digits; ++index) {
    const uint8_t shift = static_cast<uint8_t>((digits - index - 1) * 4);
    encoded[index] = kHex[(value >> shift) & 0x0f];
  }
  Serial.write(encoded, digits);
}

const char* boot_result(conduit::promicro::BootBindResult result) {
  using conduit::promicro::BootBindResult;
  switch (result) {
    case BootBindResult::kBound:
      return "bound";
    case BootBindResult::kAlreadyBound:
      return "already-bound";
    case BootBindResult::kInvalidIdentity:
      return "invalid-identity";
    case BootBindResult::kConflictingBinding:
      return "conflicting-binding";
  }
  return "invalid-result";
}

const char* activation_result(conduit::promicro::ActivationResult result) {
  using conduit::promicro::ActivationResult;
  switch (result) {
    case ActivationResult::kAdmitted:
      return "admitted";
    case ActivationResult::kAlreadyAdmitted:
      return "already-admitted";
    case ActivationResult::kBootAbsent:
      return "boot-absent";
    case ActivationResult::kInvalidIdentity:
      return "invalid-identity";
    case ActivationResult::kStaleHost:
      return "stale-host";
    case ActivationResult::kStaleBoot:
      return "stale-boot";
    case ActivationResult::kStaleOfferGeneration:
      return "stale-offer-generation";
    case ActivationResult::kConflictingActivation:
      return "conflicting-activation";
  }
  return "invalid-result";
}

void respond_status() {
  WRITE_TEXT(
      "STATUS schema=conduit.pete/promicro-brainstem@1 create_uart=isolated "
      "create_tx_bytes=");
#if defined(CONDUIT_CREATE_HIL)
  Serial.print(create_tx_bytes);
#else
  Serial.print(0);
#endif
  WRITE_TEXT(" boot_binding=");
  Serial.write(lifecycle.boot_bound() ? "bound" : "absent");
  WRITE_TEXT(" activation=");
  Serial.write(lifecycle.activation_admitted() ? "admitted" : "absent");
  WRITE_TEXT(
      " motion_authority=absent command_capacity=64 "
      "assigned_obligation_capacity=1 group_zero_bytes=26 "
#if defined(CONDUIT_CREATE_HIL)
      "create_codec=compiled-hil-isolated\n");
#else
      "create_codec=compiled-disabled\n");
#endif
}

const char* image_profile() {
  return kImageProfile == conduit::promicro::ImageProfile::kCreateHil
             ? "create-hil"
             : "isolated";
}

void respond_attestation() {
  WRITE_TEXT(
      "ATTESTATION schema=conduit.avr-promicro/image-attestation@1 "
      "build_id=");
  Serial.write(kBuildAttestation.build_id);
  WRITE_TEXT(" source_sha=");
  Serial.write(kBuildAttestation.source_sha);
  WRITE_TEXT(" source_digest_sha256=");
  Serial.write(kBuildAttestation.source_digest_sha256);
  WRITE_TEXT(" profile=");
  Serial.write(image_profile());
  WRITE_TEXT(
      " artifact_sha256_binding=build-receipt create_uart=isolated\n");
}

void respond_offer() {
  conduit::promicro::CreateObservationOffer offer{};
  const conduit::promicro::OfferResult result =
      conduit::promicro::current_offer(lifecycle, kBuildAttestation, offer);
  using conduit::promicro::OfferResult;
  if (result == OfferResult::kBootAbsent) {
    WRITE_TEXT(
        "OFFER schema=conduit.host/offer-set@1 outcome=refused "
        "reason=boot-absent count=0 create_uart=isolated\n");
    return;
  }
  if (result == OfferResult::kInvalidBuildIdentity) {
    WRITE_TEXT(
        "OFFER schema=conduit.host/offer-set@1 outcome=refused "
        "reason=invalid-build-identity count=0 create_uart=isolated\n");
    return;
  }
  if (result == OfferResult::kNoExecutableImplementation) {
    WRITE_TEXT(
        "OFFER schema=conduit.host/offer-set@1 outcome=available count=0 "
        "reason=implementation-not-in-image create_uart=isolated\n");
    return;
  }
  WRITE_TEXT(
      "OFFER schema=conduit.host/offer-set@1 outcome=available count=1 "
      "host=");
  write_hex(offer.placement.host_id.value, 8);
  WRITE_TEXT(" boot=");
  write_hex(offer.placement.boot_id.value, 8);
  WRITE_TEXT(" offer_generation=");
  write_hex(offer.placement.offer_generation, 8);
  WRITE_TEXT(
      " kind=robotics/create-group-zero-observation@1 "
      "implementation=conduit.avr/create-group-zero@1 artifact_build=");
  Serial.write(offer.artifact_build);
  WRITE_TEXT(" operation_capacity=1 response_byte_capacity=26 "
             "maximum_deadline_ms=2000 create_uart=isolated\n");
}

void respond_rx_boundary() {
  // GPIO-only sampling: USART1 remains disabled and TXO remains INPUT.
  isolate_create_uart();
  conduit::promicro::RxBoundaryEvidence evidence{};
  const uint32_t started_us = micros();
  for (uint16_t index = 0; index < conduit::promicro::kRxBoundarySamples;
       ++index) {
    evidence.push(digitalRead(conduit::promicro::kCreateRxPin) == HIGH);
  }
  const uint32_t duration_us = micros() - started_us;
  isolate_create_uart();
  WRITE_TEXT(
      "RX_BOUNDARY schema=conduit.pete/create-rx-boundary@1 "
      "outcome=sampled samples=");
  Serial.print(conduit::promicro::kRxBoundarySamples);
  WRITE_TEXT(" high=");
  Serial.print(evidence.high_samples);
  WRITE_TEXT(" low=");
  Serial.print(evidence.low_samples);
  WRITE_TEXT(" transitions=");
  Serial.print(evidence.transitions);
  WRITE_TEXT(" duration_us=");
  Serial.print(duration_us);
  WRITE_TEXT(
      " rx_pin=D0/PD2 tx_pin=D1/PD3-input usart1=disabled "
      "create_tx_bytes=0\n");
}

void respond_boot() {
  const conduit::promicro::BootBinding& binding = command.boot_binding();
  const conduit::promicro::BootBindResult result = lifecycle.bind_boot(binding);
  WRITE_TEXT("BOOT_BIND schema=conduit.host/boot-binding@1 outcome=");
  Serial.write(boot_result(result));
  WRITE_TEXT(" host=");
  write_hex(binding.host_id.value, 8);
  WRITE_TEXT(" boot=");
  write_hex(binding.boot_id.value, 8);
  WRITE_TEXT(" offer_generation=");
  write_hex(binding.offer_generation, 8);
  WRITE_TEXT(" create_uart=isolated\n");
}

void respond_activation() {
  const conduit::promicro::ObservationActivation& activation =
      command.activation();
  const conduit::promicro::ActivationResult result = lifecycle.admit(activation);
  WRITE_TEXT(
      "ACTIVATION schema=conduit.host/observation-activation@1 outcome=");
  Serial.write(activation_result(result));
  WRITE_TEXT(" host=");
  write_hex(activation.host_id.value, 8);
  WRITE_TEXT(" boot=");
  write_hex(activation.boot_id.value, 8);
  WRITE_TEXT(" offer_generation=");
  write_hex(activation.offer_generation, 8);
  WRITE_TEXT(" plan_fragment=");
  write_hex(activation.plan_fragment_id.value, 8);
  WRITE_TEXT(" operation=");
  write_hex(activation.operation_id.value, 4);
  WRITE_TEXT(" active_play=");
  write_hex(activation.active_play_id.value, 8);
  WRITE_TEXT(" authority_grant=");
  write_hex(activation.authority_grant_id.value, 8);
  WRITE_TEXT(" execution=disabled create_uart=isolated\n");
}

#if defined(CONDUIT_CREATE_HIL)
const char* hil_start_result(conduit::promicro::HilStartResult result) {
  using conduit::promicro::HilStartResult;
  switch (result) {
    case HilStartResult::kStarted:
      return "started";
    case HilStartResult::kStaleActivation:
      return "stale-activation";
    case HilStartResult::kAdmissionRefused:
      return "admission-refused";
    case HilStartResult::kAlreadyRunning:
      return "already-running";
    case HilStartResult::kAlreadyTerminal:
      return "already-terminal";
    case HilStartResult::kProviderUnavailable:
      return "provider-unavailable";
  }
  return "invalid-result";
}

void respond_execution() {
  const conduit::promicro::AssignedObligation& execution = command.execution();
  const conduit::promicro::HilStartResult result = create_executor.start(
      lifecycle, create_obligation, execution.plan_fragment_id,
      execution.operation_id, execution.deadline_ms, millis(), create_serial);
  terminal_reported = result != conduit::promicro::HilStartResult::kStarted;
  if (result == conduit::promicro::HilStartResult::kStarted) {
    create_tx_bytes = static_cast<uint8_t>(
        create_tx_bytes + conduit::promicro::kCreateSetupByteCount +
        conduit::promicro::kCreateRequestByteCount);
  }
  WRITE_TEXT("EXECUTION schema=conduit.pete/create-group-zero@1 outcome=");
  Serial.write(hil_start_result(result));
  WRITE_TEXT(" plan_fragment=");
  write_hex(execution.plan_fragment_id, 8);
  WRITE_TEXT(" operation=");
  write_hex(execution.operation_id, 4);
  WRITE_TEXT(" deadline_ms=");
  write_hex(execution.deadline_ms, 4);
  WRITE_TEXT(" setup_bytes=2 request_bytes=2 response_capacity=26\n");
}

const char* terminal_disposition(
    conduit::promicro::TerminalDisposition disposition) {
  using conduit::promicro::TerminalDisposition;
  switch (disposition) {
    case TerminalDisposition::kPending:
      return "pending";
    case TerminalDisposition::kCompleted:
      return "completed";
    case TerminalDisposition::kCancelled:
      return "cancelled";
    case TerminalDisposition::kDeadlineExpired:
      return "deadline-expired";
    case TerminalDisposition::kProviderUnavailable:
      return "provider-unavailable";
    case TerminalDisposition::kDeviceNoResponse:
      return "device-no-response";
    case TerminalDisposition::kMalformedResponse:
      return "malformed-response";
  }
  return "invalid-disposition";
}

void report_terminal_execution() {
  if (!create_executor.terminal() || terminal_reported) {
    return;
  }
  const conduit::promicro::TerminalEvidence& evidence =
      create_executor.evidence();
  WRITE_TEXT("TERMINAL schema=conduit.pete/create-group-zero@1 outcome=");
  Serial.write(terminal_disposition(evidence.disposition));
  WRITE_TEXT(" plan_fragment=");
  write_hex(evidence.plan_fragment_id, 8);
  WRITE_TEXT(" operation=");
  write_hex(evidence.operation_id, 4);
  WRITE_TEXT(" response_bytes=");
  write_hex(evidence.response_bytes, 2);
  if (evidence.payload_valid) {
    WRITE_TEXT(" payload=valid");
    const conduit::promicro::GroupZeroSample& sample =
        create_executor.sample();
    WRITE_TEXT(" bump_drop=");
    write_hex(sample.bump_and_wheel_drop, 2);
    WRITE_TEXT(" wall=");
    Serial.print(sample.wall ? 1 : 0);
    WRITE_TEXT(" cliffs=");
    write_hex(sample.cliff_bits, 2);
    WRITE_TEXT(" virtual_wall=");
    Serial.print(sample.virtual_wall ? 1 : 0);
    WRITE_TEXT(" wheel_overcurrents=");
    write_hex(sample.wheel_overcurrents, 2);
    WRITE_TEXT(" dirt=");
    write_hex(sample.dirt_detect, 4);
    WRITE_TEXT(" infrared=");
    write_hex(sample.infrared, 2);
    WRITE_TEXT(" buttons=");
    write_hex(sample.buttons, 2);
    WRITE_TEXT(" distance_mm=");
    write_hex(static_cast<uint16_t>(sample.distance_delta_mm), 4);
    WRITE_TEXT(" angle_degrees=");
    write_hex(static_cast<uint16_t>(sample.angle_delta_degrees), 4);
    WRITE_TEXT(" charging_state=");
    write_hex(sample.charging_state, 2);
    WRITE_TEXT(" millivolts=");
    write_hex(sample.millivolts, 4);
    WRITE_TEXT(" milliamps=");
    write_hex(static_cast<uint16_t>(sample.milliamps), 4);
    WRITE_TEXT(" temperature_c=");
    write_hex(static_cast<uint8_t>(sample.temperature_celsius), 2);
    WRITE_TEXT(" charge_mah=");
    write_hex(sample.charge_mah, 4);
    WRITE_TEXT(" capacity_mah=");
    write_hex(sample.capacity_mah, 4);
  } else {
    WRITE_TEXT(" payload=absent");
  }
  WRITE_TEXT(" create_uart=isolated\n");
  terminal_reported = true;
}
#endif

void respond(Request request) {
  switch (request) {
    case Request::kHello:
      WRITE_TEXT(
          "TARGET schema=conduit.target/availability@1 "
          "target_id=avr/promicro/pete-brainstem "
          "target=atmega32u4-5v-16mhz line=usb-cdc@1\n");
      break;
    case Request::kStatus:
      respond_status();
      break;
    case Request::kAttest:
      respond_attestation();
      break;
    case Request::kOffer:
      respond_offer();
      break;
    case Request::kRxBoundary:
      respond_rx_boundary();
      break;
    case Request::kBindBoot:
      respond_boot();
      break;
    case Request::kActivateObservation:
      respond_activation();
      break;
    case Request::kExecuteObservation:
#if defined(CONDUIT_CREATE_HIL)
      respond_execution();
#else
      WRITE_TEXT(
          "REFUSED schema=conduit.host/request-refusal@1 "
          "reason=create-hil-compiled-disabled create_uart=isolated\n");
#endif
      break;
    case Request::kMalformed:
      WRITE_TEXT(
          "REFUSED schema=conduit.host/request-refusal@1 reason=malformed "
          "create_uart=isolated\n");
      break;
    case Request::kOverflow:
      WRITE_TEXT(
          "REFUSED schema=conduit.host/request-refusal@1 "
          "reason=command-too-long limit=64 create_uart=isolated\n");
      break;
    case Request::kUnsupported:
      WRITE_TEXT(
          "REFUSED schema=conduit.host/request-refusal@1 reason=unsupported "
          "create_uart=isolated\n");
      break;
  }
}

}  // namespace

void setup() {
  isolate_create_uart();
  Serial.begin(115200);
}

void loop() {
  // Reassert the fail-closed physical state independently of USB activity.
#if defined(CONDUIT_CREATE_HIL)
  create_executor.tick(millis(), create_serial);
  report_terminal_execution();
  if (!create_executor.running()) {
    isolate_create_uart();
  }
#else
  isolate_create_uart();
#endif

  while (Serial.available() > 0) {
    const char byte = static_cast<char>(Serial.read());
    if (byte != '\n') {
      command.push(byte);
      continue;
    }
    respond(command.push(byte));
  }
}
