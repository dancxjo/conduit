//! Finite USB-UART GPIO adapter for the physical light-switch demo.
//!
//! GPIO9 is the DevKitM-1 BOOT button and GPIO8 is its WS2812 RGB LED. This
//! adapter reports debounced presses and manifests absolute Boolean values;
//! it deliberately owns neither toggle state nor scheduling.

#![no_std]
#![no_main]

use core::fmt::Write as _;
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Instant, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    Blocking,
    clock::CpuClock,
    gpio::{Input, InputConfig, Level, Pull},
    interrupt::software::SoftwareInterruptControl,
    rmt::{Channel, PulseCode, Rmt, Tx, TxChannelConfig, TxChannelCreator},
    rng::Rng,
    time::Rate,
    timer::timg::TimerGroup,
    uart::{Config as UartConfig, Uart},
};

const LED_OFF: &[u8] = b"CONDUIT_LIGHT_SWITCH_LED level=false\n";
const LED_ON: &[u8] = b"CONDUIT_LIGHT_SWITCH_LED level=true\n";
const DEBOUNCE: Duration = Duration::from_millis(35);
const MAXIMUM_PLAYBACK_PATTERN_BYTES: usize = 64;
const MAXIMUM_COMMAND_BYTES: usize = 2 + MAXIMUM_PLAYBACK_PATTERN_BYTES * 2;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(size: 4096);
    let timer_group = TimerGroup::new(peripherals.TIMG0);
    let software_interrupts = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timer_group.timer0, software_interrupts.software_interrupt0);
    let rng = Rng::new();
    let boot_nonce = u64::from(rng.random()) << 32 | u64::from(rng.random());

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
    report_ready(&mut uart, boot_nonce).await;
    write_all(&mut uart, LED_OFF).await;

    let mut command = [0_u8; MAXIMUM_COMMAND_BYTES];
    let mut command_len = 0_usize;
    let mut transition_sequence = 0_u64;
    loop {
        let mut incoming = [0_u8; 1];
        match select(
            button.wait_for_falling_edge(),
            uart.read_async(&mut incoming),
        )
        .await
        {
            Either::First(()) => {
                Timer::after(DEBOUNCE).await;
                if button.is_high() {
                    continue;
                }
                report_transition(
                    &mut uart,
                    "pressed",
                    transition_sequence,
                    Instant::now().as_micros(),
                )
                .await;
                transition_sequence = next_sequence(transition_sequence).await;
                loop {
                    button.wait_for_rising_edge().await;
                    Timer::after(DEBOUNCE).await;
                    if button.is_high() {
                        break;
                    }
                }
                report_transition(
                    &mut uart,
                    "released",
                    transition_sequence,
                    Instant::now().as_micros(),
                )
                .await;
                transition_sequence = next_sequence(transition_sequence).await;
            }
            Either::Second(Ok(1)) => {
                if incoming[0] == b'\n' {
                    if &command[..command_len] == b"?" {
                        report_ready(&mut uart, boot_nonce).await;
                        command_len = 0;
                        continue;
                    }
                    match apply_command(&command[..command_len], led).await {
                        Ok((next, message)) => {
                            led = next;
                            write_all(&mut uart, message).await;
                        }
                        Err((next, reason)) => {
                            led = next;
                            write_all(&mut uart, reason).await;
                        }
                    }
                    command_len = 0;
                } else if command_len < command.len() {
                    command[command_len] = incoming[0];
                    command_len += 1;
                } else {
                    command_len = 0;
                    write_all(
                        &mut uart,
                        b"CONDUIT_MORSE_KEY_PLAYBACK outcome=refused reason=frame-bound final-led=false\n",
                    )
                    .await;
                }
            }
            Either::Second(Ok(_)) => {}
            Either::Second(Err(_)) => core::future::pending::<()>().await,
        }
    }
}

async fn report_ready(uart: &mut Uart<'_, esp_hal::Async>, boot_nonce: u64) {
    let mut line = heapless::String::<192>::new();
    if writeln!(
        line,
        "CONDUIT_LIGHT_SWITCH_READY host=esp32-c3/devkitm-1 boot={boot_nonce:016x} button=gpio9 led=gpio8 transitions=pressed-released clock=boot-monotonic-us@1"
    )
    .is_err()
    {
        core::future::pending::<()>().await;
    }
    write_all(uart, line.as_bytes()).await;
}

async fn report_transition(
    uart: &mut Uart<'_, esp_hal::Async>,
    phase: &str,
    sequence: u64,
    monotonic_micros: u64,
) {
    let mut line = heapless::String::<160>::new();
    if writeln!(
        line,
        "CONDUIT_LIGHT_SWITCH_BUTTON transition={phase} sequence={sequence} monotonic-us={monotonic_micros}"
    )
    .is_err()
    {
        core::future::pending::<()>().await;
    }
    write_all(uart, line.as_bytes()).await;
}

async fn next_sequence(sequence: u64) -> u64 {
    match sequence.checked_add(1) {
        Some(next) => next,
        None => core::future::pending::<u64>().await,
    }
}

async fn apply_command<'d>(
    command: &[u8],
    mut led: Channel<'d, Blocking, Tx>,
) -> Result<(Channel<'d, Blocking, Tx>, &'static [u8]), (Channel<'d, Blocking, Tx>, &'static [u8])>
{
    if command == b"0" || command == b"1" {
        let level = command[0] == b'1';
        led = set_led(led, level);
        return Ok((led, if level { LED_ON } else { LED_OFF }));
    }
    let Some(hex) = command.strip_prefix(b"M") else {
        led = set_led(led, false);
        return Err((led, playback_refusal()));
    };
    if hex.is_empty() || hex.len() % 2 != 0 || hex.len() / 2 > MAXIMUM_PLAYBACK_PATTERN_BYTES {
        led = set_led(led, false);
        return Err((led, playback_refusal()));
    }
    let mut encoded = [0_u8; MAXIMUM_PLAYBACK_PATTERN_BYTES];
    for (index, pair) in hex.chunks_exact(2).enumerate() {
        let Some(high) = hex_nibble(pair[0]) else {
            led = set_led(led, false);
            return Err((led, playback_refusal()));
        };
        let Some(low) = hex_nibble(pair[1]) else {
            led = set_led(led, false);
            return Err((led, playback_refusal()));
        };
        encoded[index] = high << 4 | low;
    }
    let pattern = match conduit_text::MorsePattern::decode(&encoded[..hex.len() / 2]) {
        Ok(pattern) => pattern,
        Err(_) => {
            led = set_led(led, false);
            return Err((led, playback_refusal()));
        }
    };
    for segment in pattern.segments {
        led = set_led(led, segment.level);
        Timer::after(Duration::from_millis(
            u64::from(pattern.unit_millis) * u64::from(segment.units),
        ))
        .await;
    }
    led = set_led(led, false);
    Ok((
        led,
        b"CONDUIT_MORSE_KEY_PLAYBACK outcome=completed final-led=false\n",
    ))
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

const fn playback_refusal() -> &'static [u8] {
    b"CONDUIT_MORSE_KEY_PLAYBACK outcome=refused reason=malformed-pattern final-led=false\n"
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

async fn write_all(uart: &mut Uart<'_, esp_hal::Async>, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        match uart.write_async(bytes).await {
            Ok(0) | Err(_) => core::future::pending::<()>().await,
            Ok(written) => bytes = &bytes[written..],
        }
    }
}
