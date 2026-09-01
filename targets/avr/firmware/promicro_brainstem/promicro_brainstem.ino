#include "protocol.h"
#include "embedded_spore.h"

#include <avr/pgmspace.h>

using conduit::promicro::CommandBuffer;
using conduit::promicro::Request;

#if !defined(CONDUIT_AVR_BUILD_ID) || !defined(CONDUIT_AVR_SOURCE_SHA) || \
    !defined(CONDUIT_AVR_SOURCE_DIGEST)
#error "AVR build identity must be supplied by cargo xtask"
#endif

#define WRITE_TEXT(text) Serial.print(F(text))

namespace {

constexpr uint8_t kCreateRxPin = 0;
constexpr uint8_t kCreateTxPin = 1;
constexpr uint8_t kPowerTogglePin = 4;
constexpr uint8_t kChargingInputPin = 5;

CommandBuffer command;

uint8_t read_spore(uint16_t offset) {
  return pgm_read_byte_near(conduit::promicro::kSporeRegionStart + offset);
}

void write_spore_field(uint8_t field) {
  conduit::promicro::EmbeddedSporeField selected{};
  if (!conduit::promicro::embedded_spore_field(read_spore, field, &selected)) {
    return;
  }
  for (uint8_t index = 0; index < selected.length; ++index) {
    Serial.write(read_spore(selected.offset + index));
  }
}

void isolate_create_outputs() {
  Serial1.end();
  pinMode(kCreateRxPin, INPUT);
  pinMode(kCreateTxPin, INPUT);
  pinMode(kPowerTogglePin, INPUT);
  pinMode(kChargingInputPin, INPUT);
}

void respond(Request request) {
  switch (request) {
    case Request::kHello:
      WRITE_TEXT(
          "TARGET target=atmega32u4-5v-16mhz line=usb-cdc "
          "runtime=uninstalled\n");
      break;
    case Request::kStatus:
      WRITE_TEXT(
          "STATUS create_uart=isolated create_tx_bytes=0 "
          "power_toggle=D4/input charging_input=D5/input "
          "assigned_plan=absent runtime=uninstalled\n");
      break;
    case Request::kAttest:
      WRITE_TEXT("ATTEST build_id=" CONDUIT_AVR_BUILD_ID " source_sha="
                 CONDUIT_AVR_SOURCE_SHA " source_digest_sha256="
                 CONDUIT_AVR_SOURCE_DIGEST " profile=isolated ");
      if (conduit::promicro::embedded_spore_valid(read_spore)) {
        WRITE_TEXT("spore=present spore_id=");
        write_spore_field(0);
        WRITE_TEXT(" image_id=");
        write_spore_field(1);
        WRITE_TEXT(" invitation_id=");
        write_spore_field(2);
        WRITE_TEXT(" body_id=");
        write_spore_field(3);
        Serial.write('\n');
      } else {
        WRITE_TEXT("spore=absent\n");
      }
      break;
    case Request::kOverflow:
      WRITE_TEXT("REFUSED reason=command-too-long limit=64\n");
      break;
    case Request::kUnsupported:
      WRITE_TEXT("REFUSED reason=unsupported\n");
      break;
  }
}

}  // namespace

void setup() {
  isolate_create_outputs();
  Serial.begin(115200);
}

void loop() {
  isolate_create_outputs();
  while (Serial.available() > 0) {
    const char byte = static_cast<char>(Serial.read());
    if (byte != '\n') {
      command.push(byte);
      continue;
    }
    respond(command.push(byte));
  }
}
