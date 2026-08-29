//! Finite USB-UART GPIO adapter for the physical light-switch demo.
//!
//! GPIO9 is the DevKitM-1 BOOT button and GPIO8 is its WS2812 RGB LED. This
//! adapter reports debounced presses and manifests absolute Boolean values;
//! it deliberately owns neither toggle state nor scheduling.

#![no_std]
#![no_main]

use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    Blocking,
    clock::CpuClock,
    gpio::{Input, InputConfig, Level, Pull},
    interrupt::software::SoftwareInterruptControl,
    rmt::{Channel, PulseCode, Rmt, Tx, TxChannelConfig, TxChannelCreator},
    time::Rate,
    timer::timg::TimerGroup,
    uart::{Config as UartConfig, Uart},
};

const READY: &[u8] = b"CONDUIT_LIGHT_SWITCH_READY host=esp32-c3/devkitm-1 button=gpio9 led=gpio8\n";
const BUTTON: &[u8] = b"CONDUIT_LIGHT_SWITCH_BUTTON transition=pressed\n";
const LED_OFF: &[u8] = b"CONDUIT_LIGHT_SWITCH_LED level=false\n";
const LED_ON: &[u8] = b"CONDUIT_LIGHT_SWITCH_LED level=true\n";
const DEBOUNCE: Duration = Duration::from_millis(35);

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(size: 4096);
    let timer_group = TimerGroup::new(peripherals.TIMG0);
    let software_interrupts = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timer_group.timer0, software_interrupts.software_interrupt0);

    let mut button = Input::new(
        peripherals.GPIO9,
        InputConfig::default().with_pull(Pull::Up),
    );
    let mut uart = Uart::new(peripherals.UART0, UartConfig::default())
        .expect("the inspected C3 UART must initialize")
        .with_rx(peripherals.GPIO20)
        .with_tx(peripherals.GPIO21)
        .into_async();
    let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80))
        .expect("the inspected C3 RMT must initialize");
    let led = rmt
        .channel0
        .configure_tx(
            &TxChannelConfig::default()
                .with_clk_divider(1)
                .with_idle_output_level(Level::Low)
                .with_idle_output(true)
                .with_carrier_modulation(false),
        )
        .expect("the C3 RGB channel must configure")
        .with_pin(peripherals.GPIO8);
    let mut led = set_led(led, false);
    write_all(&mut uart, READY).await;
    write_all(&mut uart, LED_OFF).await;

    let mut command = [0_u8; 2];
    let mut command_len = 0_usize;
    loop {
        let mut incoming = [0_u8; 1];
        match select(button.wait_for_falling_edge(), uart.read_async(&mut incoming)).await {
            Either::First(()) => {
                write_all(&mut uart, BUTTON).await;
                Timer::after(DEBOUNCE).await;
                while button.is_low() {
                    Timer::after(Duration::from_millis(5)).await;
                }
            }
            Either::Second(Ok(1)) => match incoming[0] {
                b'0' | b'1' if command_len == 0 => {
                    command[0] = incoming[0];
                    command_len = 1;
                }
                b'\n' if command_len == 1 => {
                    let level = command[0] == b'1';
                    led = set_led(led, level);
                    write_all(&mut uart, if level { LED_ON } else { LED_OFF }).await;
                    command_len = 0;
                }
                _ => command_len = 0,
            },
            Either::Second(Ok(_)) => {}
            Either::Second(Err(_)) => core::future::pending::<()>().await,
        }
    }
}

fn set_led<'d>(led: Channel<'d, Blocking, Tx>, level: bool) -> Channel<'d, Blocking, Tx> {
    // One WS2812 value in GRB order at an 80 MHz RMT clock. Bright green is
    // intentionally capped at 24/255 for a small indicator LED.
    let bytes = if level { [24_u8, 0, 0] } else { [0_u8; 3] };
    let zero = PulseCode::new(Level::High, 32, Level::Low, 68);
    let one = PulseCode::new(Level::High, 68, Level::Low, 32);
    let mut pulses = [PulseCode::end_marker(); 25];
    let mut index = 0;
    for byte in bytes {
        for mask in [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01] {
            pulses[index] = if byte & mask == 0 { zero } else { one };
            index += 1;
        }
    }
    pulses[24] = PulseCode::end_marker();
    led.transmit(&pulses)
        .expect("one fixed RGB frame must fit one RMT block")
        .wait()
        .expect("the admitted RGB transmission must complete")
}

async fn write_all(
    uart: &mut Uart<'_, esp_hal::Async>,
    mut bytes: &[u8],
) {
    while !bytes.is_empty() {
        match uart.write_async(bytes).await {
            Ok(0) | Err(_) => core::future::pending::<()>().await,
            Ok(written) => bytes = &bytes[written..],
        }
    }
}
