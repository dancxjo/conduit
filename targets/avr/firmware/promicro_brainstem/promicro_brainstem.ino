#include "assigned_obligations.h"
#include "create_oi.h"
#include "group_zero.h"
#include "lifecycle.h"
#include "protocol.h"

using conduit::promicro::CommandBuffer;
using conduit::promicro::Request;

namespace {

// SparkFun Pro Micro hardware UART pins. The initial image never initializes
// Serial1 and holds both pins as high-impedance inputs. In particular, TXO must
// not emit a Create OI byte merely because the board boots or USB reconnects.
constexpr uint8_t kCreateRxPin = 0;
constexpr uint8_t kCreateTxPin = 1;

constexpr char kHello[] =
    "TARGET schema=conduit.target/availability@1 "
    "target_id=avr/promicro/pete-brainstem "
    "target=atmega32u4-5v-16mhz line=usb-cdc@1\n";
constexpr char kRefused[] =
    "REFUSED schema=conduit.host/request-refusal@1 reason=unsupported "
    "create_uart=isolated\n";
constexpr char kOverflow[] =
    "REFUSED schema=conduit.host/request-refusal@1 reason=command-too-long "
    "limit=64 create_uart=isolated\n";
constexpr char kMalformed[] =
    "REFUSED schema=conduit.host/request-refusal@1 reason=malformed "
    "create_uart=isolated\n";

CommandBuffer command;
conduit::promicro::BrainstemLifecycle lifecycle;

void isolate_create_uart() {
  Serial1.end();
  pinMode(kCreateRxPin, INPUT);
  pinMode(kCreateTxPin, INPUT);
}

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
  Serial.write(
      "STATUS schema=conduit.pete/promicro-brainstem@1 create_uart=isolated "
      "create_tx_bytes=0 boot_binding=");
  Serial.write(lifecycle.boot_bound() ? "bound" : "absent");
  Serial.write(" activation=");
  Serial.write(lifecycle.activation_admitted() ? "admitted" : "absent");
  Serial.write(
      " motion_authority=absent command_capacity=64 "
      "assigned_obligation_capacity=1 group_zero_bytes=26 "
      "create_codec=compiled-disabled\n");
}

void respond_boot() {
  const conduit::promicro::BootBinding& binding = command.boot_binding();
  const conduit::promicro::BootBindResult result = lifecycle.bind_boot(binding);
  Serial.write("BOOT_BIND schema=conduit.host/boot-binding@1 outcome=");
  Serial.write(boot_result(result));
  Serial.write(" host=");
  write_hex(binding.host_id.value, 8);
  Serial.write(" boot=");
  write_hex(binding.boot_id.value, 8);
  Serial.write(" offer_generation=");
  write_hex(binding.offer_generation, 8);
  Serial.write(" create_uart=isolated\n");
}

void respond_activation() {
  const conduit::promicro::ObservationActivation& activation =
      command.activation();
  const conduit::promicro::ActivationResult result = lifecycle.admit(activation);
  Serial.write(
      "ACTIVATION schema=conduit.host/observation-activation@1 outcome=");
  Serial.write(activation_result(result));
  Serial.write(" host=");
  write_hex(activation.host_id.value, 8);
  Serial.write(" boot=");
  write_hex(activation.boot_id.value, 8);
  Serial.write(" offer_generation=");
  write_hex(activation.offer_generation, 8);
  Serial.write(" plan_fragment=");
  write_hex(activation.plan_fragment_id.value, 8);
  Serial.write(" operation=");
  write_hex(activation.operation_id.value, 4);
  Serial.write(" active_play=");
  write_hex(activation.active_play_id.value, 8);
  Serial.write(" authority_grant=");
  write_hex(activation.authority_grant_id.value, 8);
  Serial.write(" execution=disabled create_uart=isolated\n");
}

void respond(Request request) {
  switch (request) {
    case Request::kHello:
      Serial.write(kHello);
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
    case Request::kMalformed:
      Serial.write(kMalformed);
      break;
    case Request::kOverflow:
      Serial.write(kOverflow);
      break;
    case Request::kUnsupported:
      Serial.write(kRefused);
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
  isolate_create_uart();

  while (Serial.available() > 0) {
    const char byte = static_cast<char>(Serial.read());
    if (byte != '\n') {
      command.push(byte);
      continue;
    }
    respond(command.push(byte));
  }
}
