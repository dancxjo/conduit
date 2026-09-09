// Installed provisioning contributions. This catalog does not admit a Host.
import { createPhysicalHostTargetCatalog } from "./creche-target-catalog.mjs";
import { AVR_PRO_MICRO_CRECHE_TARGET_CONTRIBUTION } from "../../../targets/avr/deployment/browser/creche-adapter.mjs";
import { RP2040_CRECHE_TARGET_CONTRIBUTION } from "../../../targets/rp2040/deployment/browser/creche-adapter.mjs";
import { ESP32_CRECHE_TARGET_CONTRIBUTIONS } from "../../../targets/esp32/deployment/browser/creche-adapter.mjs";
import { STD_EXISTING_COMPUTER_CONTRIBUTIONS } from "../../../targets/std/deployment/browser/creche-adapter.mjs";
import { BROWSER_EXISTING_COMPUTER_CONTRIBUTION } from "../../../targets/browser/deployment/browser/creche-adapter.mjs";
import { ORANGE_PI_CRECHE_TARGET_CONTRIBUTION } from "../../../targets/orange-pi/deployment/browser/creche-adapter.mjs";
import { RASPBERRY_PI_CRECHE_TARGET_CONTRIBUTIONS } from "../../../targets/raspberry-pi/deployment/browser/creche-adapter.mjs";
import { CONDUITOS_CRECHE_TARGET_CONTRIBUTIONS } from "../../../targets/conduitos/deployment/browser/creche-adapter.mjs";

export function createInstalledCrecheTargetCatalog() {
return createPhysicalHostTargetCatalog({
  generation: 1,
  contributions: [
    RP2040_CRECHE_TARGET_CONTRIBUTION,
    AVR_PRO_MICRO_CRECHE_TARGET_CONTRIBUTION,
    ...ESP32_CRECHE_TARGET_CONTRIBUTIONS,
    ...STD_EXISTING_COMPUTER_CONTRIBUTIONS,
    BROWSER_EXISTING_COMPUTER_CONTRIBUTION,
    ORANGE_PI_CRECHE_TARGET_CONTRIBUTION,
    ...RASPBERRY_PI_CRECHE_TARGET_CONTRIBUTIONS,
    ...CONDUITOS_CRECHE_TARGET_CONTRIBUTIONS,
  ],
});
}
