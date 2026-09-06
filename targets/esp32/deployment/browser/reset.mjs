export class Esp32ResetRefusal extends Error {
  constructor(code, message, cause = undefined) {
    super(message, cause ? { cause } : undefined);
    this.name = "Esp32ResetRefusal";
    this.code = code;
  }
}

function refuse(code, message, cause) {
  throw new Esp32ResetRefusal(code, message, cause);
}

const delay = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

async function apply(base, signals, wait, milliseconds) {
  await base.setSignals(signals);
  if (milliseconds > 0) await wait(milliseconds);
}

export async function enterEsp32RomLoader({ base, strategy, wait = delay }) {
  if (!base || typeof base.setSignals !== "function") {
    refuse("BaseContract", "ESP32 reset requires admitted serial modem-control operations");
  }
  let operations = 0;
  const set = async (signals, milliseconds = 0) => {
    await apply(base, signals, wait, milliseconds);
    operations += 1;
  };
  try {
    if (strategy === "classic") {
      await set({ dataTerminalReady: false });
      await set({ requestToSend: true }, 100);
      await set({ dataTerminalReady: true });
      await set({ requestToSend: false }, 50);
      await set({ dataTerminalReady: false });
    } else if (strategy === "usb-jtag") {
      await set({ requestToSend: false });
      await set({ dataTerminalReady: false }, 100);
      await set({ dataTerminalReady: true, requestToSend: false }, 100);
      await set({ dataTerminalReady: false, requestToSend: true }, 100);
      await set({ dataTerminalReady: false, requestToSend: false });
    } else {
      refuse("ResetStrategy", "ESP32 reset strategy is not supported");
    }
  } catch (error) {
    if (error instanceof Esp32ResetRefusal) throw error;
    refuse("ResetFailed", "ESP32 did not enter its ROM loader", error);
  }
  return Object.freeze({ strategy, operations });
}

export const ESP32_RESET_OPERATION_COUNTS = Object.freeze({ classic: 5, "usb-jtag": 5 });
