#pragma once

#include <stdint.h>

namespace conduit::promicro {

// Required wiring, not evidence that an installed cable has been qualified.
constexpr char kCreateAttachmentContract[] =
    "sparkfun-promicro-5v16-create1-minidin-uart@1";
constexpr uint8_t kCreateRxPin = 0;
constexpr char kCreateRxBoardLabel[] = "RXI";
constexpr char kCreateRxMcuPad[] = "PD2/RX1";
constexpr uint8_t kCreateTxMiniDinPin = 4;
constexpr uint8_t kCreateTxPin = 1;
constexpr char kCreateTxBoardLabel[] = "TXO";
constexpr char kCreateTxMcuPad[] = "PD3/TX1";
constexpr uint8_t kCreateRxMiniDinPin = 3;
constexpr uint8_t kCreateGroundMiniDinPin = 6;
constexpr uint8_t kCreateAlternateGroundMiniDinPin = 7;
constexpr uint32_t kCreateBaud = 57600;
constexpr uint8_t kCreateDataBits = 8;
constexpr uint8_t kCreateStopBits = 1;
constexpr bool kCreateParityEnabled = false;
constexpr bool kCreateFlowControlEnabled = false;
constexpr bool kCreateVpwrConnected = false;
constexpr bool kCreateBrcConnected = false;
constexpr bool kBootAndTerminalHighImpedance = true;

static_assert(kCreateRxPin == 0, "Pro Micro RXI must remain Arduino D0");
static_assert(kCreateTxPin == 1, "Pro Micro TXO must remain Arduino D1");
static_assert(kCreateRxMiniDinPin == 3,
              "Pro Micro TXO must cross to Create RXD pin 3");
static_assert(kCreateTxMiniDinPin == 4,
              "Create TXD pin 4 must cross to Pro Micro RXI");
static_assert(kCreateDataBits == 8 && kCreateStopBits == 1 &&
                  !kCreateParityEnabled && !kCreateFlowControlEnabled,
              "Create 1 UART must remain 57600 8N1 without flow control");
static_assert(!kCreateVpwrConnected && !kCreateBrcConnected,
              "initial attachment must leave Vpwr and BRC disconnected");
static_assert(kBootAndTerminalHighImpedance,
              "Create UART must isolate at boot and every terminal path");

}  // namespace conduit::promicro
