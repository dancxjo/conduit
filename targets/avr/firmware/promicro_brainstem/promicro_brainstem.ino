#include "assigned_obligations.h"
#include "create_oi.h"
#include "create_hil.h"
#include "group_zero.h"
#include "lifecycle.h"
#include "protocol.h"

using conduit::promicro::CommandBuffer;
using conduit::promicro::Request;

#define WRITE_TEXT(text) Serial.print(F(text))

namespace {

// SparkFun Pro Micro hardware UART pins. The initial image never initializes
// Serial1 and holds both pins as high-impedance inputs. In particular, TXO must
// not emit a Create OI byte merely because the board boots or USB reconnects.
constexpr uint8_t kCreateRxPin = 0;
constexpr uint8_t kCreateTxPin = 1;

CommandBuffer command;
conduit::promicro::BrainstemLifecycle lifecycle;

void isolate_create_uart() {
  Serial1.end();
  pinMode(kCreateRxPin, INPUT);
  pinMode(kCreateTxPin, INPUT);
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
      "create_tx_bytes=0 boot_binding=");
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
