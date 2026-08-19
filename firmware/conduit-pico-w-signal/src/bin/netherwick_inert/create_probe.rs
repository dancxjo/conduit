//! Explicit bounded non-actuating Create OI qualification transaction.

use core::fmt::Write as _;

use conduit_create_oi::{
    decode_sensor_packet, encode_pause_stream, encode_query_sensor, encode_start, write_command,
    CreateOiFailure, CreateUartProvider, UartProfile,
};
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::{PIN_0, PIN_1, UART0};
use embassy_rp::uart::{Blocking, Config, DataBits, Parity, StopBits, Uart};
use embassy_rp::Peri;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_nb::serial::Read as _;
use heapless::String;

use super::{send_control_frame, InertCdc};

const RESPONSE_BYTES: usize = 512;
const QUERY_DEADLINE_MS: u64 = 750;
const GROUP_ZERO_PACKET: u8 = 0;
const PREQUERY_DRAIN_LIMIT: usize = 64;

struct Provider {
    uart: Uart<'static, Blocking>,
}

type UartResources = (
    Peri<'static, UART0>,
    Peri<'static, PIN_0>,
    Peri<'static, PIN_1>,
);

pub struct Resources {
    peripherals: Option<UartResources>,
    provider: Option<Provider>,
}

impl Resources {
    pub fn new(
        uart0: Peri<'static, UART0>,
        tx: Peri<'static, PIN_0>,
        rx: Peri<'static, PIN_1>,
    ) -> Self {
        Self {
            peripherals: Some((uart0, tx, rx)),
            provider: None,
        }
    }

    pub fn is_available(&self) -> bool {
        self.peripherals.is_some() || self.provider.is_some()
    }

    fn provider(&mut self) -> &mut Provider {
        if self.provider.is_none() {
            let (uart0, tx, rx) = self
                .peripherals
                .take()
                .expect("finite UART resources initialize exactly once");
            let mut config = Config::default();
            config.baudrate = conduit_create_oi::CREATE_OI_BAUD;
            config.data_bits = DataBits::DataBits8;
            config.stop_bits = StopBits::STOP1;
            config.parity = Parity::ParityNone;
            rp_pac::PADS_BANK0.gpio(1).modify(|value| {
                value.set_pue(true);
                value.set_pde(false);
            });
            self.provider = Some(Provider {
                uart: Uart::new_blocking(uart0, tx, rx, config),
            });
        }
        self.provider
            .as_mut()
            .expect("provider was initialized above")
    }
}

enum ProbeFailure {
    Protocol(CreateOiFailure),
    Uart(embassy_rp::uart::Error),
    DeviceNoResponse,
    TruncatedFrame,
}

impl CreateUartProvider for Provider {
    type Error = embassy_rp::uart::Error;

    fn is_available(&self) -> bool {
        true
    }

    fn profile(&self) -> UartProfile {
        UartProfile::CREATE_OI
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.uart.blocking_write(bytes)
    }

    fn read_byte(&mut self, deadline_tick: u64) -> Result<Option<u8>, Self::Error> {
        let _ = deadline_tick;
        match self.uart.read() {
            Ok(byte) => Ok(Some(byte)),
            Err(nb::Error::WouldBlock) => Ok(None),
            Err(nb::Error::Other(error)) => Err(error),
        }
    }
}

async fn read_group_zero(provider: &mut Provider) -> Result<[u8; 26], ProbeFailure> {
    let mut payload = [0_u8; 26];
    let mut received = 0;
    let deadline = Instant::now() + Duration::from_millis(QUERY_DEADLINE_MS);
    while received < payload.len() && Instant::now() < deadline {
        match provider.uart.read() {
            Ok(byte) => {
                payload[received] = byte;
                received += 1;
            }
            Err(nb::Error::WouldBlock) => Timer::after(Duration::from_millis(1)).await,
            Err(nb::Error::Other(error)) => return Err(ProbeFailure::Uart(error)),
        }
    }
    match received {
        0 => Err(ProbeFailure::DeviceNoResponse),
        26 => Ok(payload),
        _ => Err(ProbeFailure::TruncatedFrame),
    }
}

fn drain_prequery_rx(provider: &mut Provider) -> (usize, usize) {
    let mut bytes = 0;
    let mut errors = 0;
    for _ in 0..PREQUERY_DRAIN_LIMIT {
        match provider.uart.read() {
            Ok(_) => bytes += 1,
            Err(nb::Error::WouldBlock) => break,
            Err(nb::Error::Other(_)) => errors += 1,
        }
    }
    (bytes, errors)
}

fn protocol_failure_name(failure: CreateOiFailure) -> &'static str {
    match failure {
        CreateOiFailure::ProviderUnavailable => "provider_unavailable",
        CreateOiFailure::WrongUartProfile { .. } => "wrong_uart_profile",
        CreateOiFailure::WriteFailed => "write_failed",
        CreateOiFailure::ReadFailed => "read_failed",
        CreateOiFailure::Timeout => "timeout",
        CreateOiFailure::DeviceNoResponse => "device_no_response",
        CreateOiFailure::UnsupportedPacket(_) => "unsupported_packet",
        CreateOiFailure::TruncatedFrame => "truncated_frame",
        CreateOiFailure::MalformedFrame => "malformed_frame",
    }
}

fn failure_name(failure: ProbeFailure) -> &'static str {
    match failure {
        ProbeFailure::Protocol(failure) => protocol_failure_name(failure),
        ProbeFailure::Uart(embassy_rp::uart::Error::Overrun) => "uart_overrun",
        ProbeFailure::Uart(embassy_rp::uart::Error::Break) => "uart_break",
        ProbeFailure::Uart(embassy_rp::uart::Error::Parity) => "uart_parity",
        ProbeFailure::Uart(embassy_rp::uart::Error::Framing) => "uart_framing",
        ProbeFailure::Uart(_) => "uart_other",
        ProbeFailure::DeviceNoResponse => "device_no_response",
        ProbeFailure::TruncatedFrame => "truncated_frame",
    }
}

pub async fn run(
    class: &mut InertCdc,
    resources: &mut Resources,
    translator_oe: &mut Output<'static>,
) {
    let provider = resources.provider();
    translator_oe.set_high();
    Timer::after(Duration::from_millis(10)).await;
    let (mut prequery_bytes, mut prequery_errors) = drain_prequery_rx(provider);

    let result = match write_command(provider, &encode_start()) {
        Err(failure) => Err(ProbeFailure::Protocol(failure)),
        Ok(()) => match write_command(provider, &encode_pause_stream()) {
            Err(failure) => Err(ProbeFailure::Protocol(failure)),
            Ok(()) => {
                Timer::after(Duration::from_millis(20)).await;
                let (tail_bytes, tail_errors) = drain_prequery_rx(provider);
                prequery_bytes += tail_bytes;
                prequery_errors += tail_errors;
                match encode_query_sensor(GROUP_ZERO_PACKET)
                    .and_then(|query| write_command(provider, &query))
                {
                    Err(failure) => Err(ProbeFailure::Protocol(failure)),
                    Ok(()) => read_group_zero(provider)
                        .await
                        .and_then(|payload| {
                            decode_sensor_packet(GROUP_ZERO_PACKET, &payload)
                                .map_err(ProbeFailure::Protocol)
                        }),
                }
            }
        },
    };
    translator_oe.set_low();

    let mut response = String::<RESPONSE_BYTES>::new();
    match result {
        Ok(packet) => {
            let bytes = packet.bytes();
            let _ = write!(
                response,
                concat!(
                    "{{\"schema\":\"conduit.netherwick/create-probe@1\",",
                    "\"success\":true,\"build_id\":\"{}\",",
                    "\"uart\":{{\"controller\":0,\"tx_gpio\":0,\"rx_gpio\":1,\"baud\":57600,\"data_bits\":8,\"stop_bits\":1,\"parity\":\"none\"}},",
                    "\"prequery_bytes_discarded\":{},\"prequery_errors_discarded\":{},",
                    "\"packet_id\":0,\"packet_bytes\":{},\"bump_wheel_drop_bits\":{},",
                    "\"charging_state\":{},\"translator_final\":\"low\",\"motion_opcode_sent\":false}}"
                ),
                env!("CONDUIT_NETHERWICK_INERT_BUILD_ID"),
                prequery_bytes,
                prequery_errors,
                bytes.len(),
                bytes[0],
                bytes[16],
            );
        }
        Err(failure) => {
            let _ = write!(
                response,
                "{{\"schema\":\"conduit.netherwick/create-probe@1\",\"success\":false,\"build_id\":\"{}\",\"failure\":\"{}\",\"prequery_bytes_discarded\":{},\"prequery_errors_discarded\":{},\"translator_final\":\"low\",\"motion_opcode_sent\":false}}",
                env!("CONDUIT_NETHERWICK_INERT_BUILD_ID"),
                failure_name(failure),
                prequery_bytes,
                prequery_errors,
            );
        }
    }
    let _ = send_control_frame(class, response.as_bytes()).await;
}
