#include "assigned_obligations.h"
#include "create_oi.h"
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
    "HOST schema=conduit.host/boot@1 host=avr/promicro/pete-brainstem "
    "target=atmega32u4-5v-16mhz line=usb-cdc@1\n";
constexpr char kStatus[] =
    "STATUS schema=conduit.pete/promicro-brainstem@1 create_uart=isolated "
    "create_tx_bytes=0 motion_authority=absent command_capacity=64 "
    "assigned_obligation_capacity=1 create_codec=compiled-disabled\n";
constexpr char kRefused[] =
    "REFUSED schema=conduit.host/request-refusal@1 reason=unsupported "
    "create_uart=isolated\n";
constexpr char kOverflow[] =
    "REFUSED schema=conduit.host/request-refusal@1 reason=command-too-long "
    "limit=64 create_uart=isolated\n";

CommandBuffer command;

void isolate_create_uart() {
  Serial1.end();
  pinMode(kCreateRxPin, INPUT);
  pinMode(kCreateTxPin, INPUT);
}

void respond(Request request) {
  switch (request) {
    case Request::kHello:
      Serial.write(kHello);
      break;
    case Request::kStatus:
      Serial.write(kStatus);
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
