//! Finite TM1637 electrical transport, called by an admitted Host operation.
//! Command meaning and register mapping belong upstream, not in this driver.
#![no_std]

/// Selected GPIO provider. `true` releases a line; it must never drive it high.
/// Each method must finish within its admitted bound. The provider owns pin
/// authority, voltage translation, timing realization and shifter enable.
pub trait OpenDrain {
    type Error;
    fn clock(&mut self, released: bool) -> Result<(), Self::Error>;
    fn data(&mut self, released: bool) -> Result<(), Self::Error>;
    fn read_data(&mut self) -> Result<bool, Self::Error>;
    /// One admitted half-period (at least 5 microseconds for this profile).
    fn half_period(&mut self) -> Result<(), Self::Error>;
}

/// A borrowed Source-encoded packet. No allocation or command synthesis.
pub struct Packet<'a> {
    pub bytes: &'a [u8],
}

#[derive(Debug, PartialEq, Eq)]
pub enum Failure<E> {
    InvalidGeometry,
    Cancelled,
    Nack { packet: u8, byte: u8 },
    Provider(E),
}

/// A transaction admits at most three packets, each containing 1..=7 bytes.
/// Cancellation is observed before START and before each byte. On any terminal
/// outcome both lines are released; cleanup is best effort after provider loss.
/// No retry, clock source, scheduling policy or ambient GPIO is introduced.
pub fn transmit<P: OpenDrain>(
    pins: &mut P,
    packets: &[Packet<'_>],
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), Failure<P::Error>> {
    if packets.is_empty()
        || packets.len() > 3
        || packets
            .iter()
            .any(|p| p.bytes.is_empty() || p.bytes.len() > 7)
    {
        return Err(Failure::InvalidGeometry);
    }
    let result = send(pins, packets, &mut cancelled);
    // Attempt both releases even if the first fails. Preserve the original
    // failure; otherwise a failed release is itself a provider failure.
    let data = pins.data(true);
    let clock = pins.clock(true);
    result?;
    data.map_err(Failure::Provider)?;
    clock.map_err(Failure::Provider)
}

fn send<P: OpenDrain>(
    pins: &mut P,
    packets: &[Packet<'_>],
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), Failure<P::Error>> {
    for (packet, contents) in packets.iter().enumerate() {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        start(pins).map_err(Failure::Provider)?;
        for (byte, value) in contents.bytes.iter().copied().enumerate() {
            if cancelled() {
                stop(pins).map_err(Failure::Provider)?;
                return Err(Failure::Cancelled);
            }
            if !write_byte(pins, value).map_err(Failure::Provider)? {
                stop(pins).map_err(Failure::Provider)?;
                return Err(Failure::Nack {
                    packet: packet as u8,
                    byte: byte as u8,
                });
            }
        }
        stop(pins).map_err(Failure::Provider)?;
    }
    Ok(())
}

fn start<P: OpenDrain>(p: &mut P) -> Result<(), P::Error> {
    p.data(true)?;
    p.clock(true)?;
    p.half_period()?;
    p.data(false)?;
    p.half_period()?;
    p.clock(false)
}

fn stop<P: OpenDrain>(p: &mut P) -> Result<(), P::Error> {
    p.clock(false)?;
    p.data(false)?;
    p.half_period()?;
    p.clock(true)?;
    p.half_period()?;
    p.data(true)?;
    p.half_period()
}

fn write_byte<P: OpenDrain>(p: &mut P, byte: u8) -> Result<bool, P::Error> {
    for bit in 0..8 {
        p.data(byte & (1 << bit) != 0)?;
        p.half_period()?;
        p.clock(true)?;
        p.half_period()?;
        p.clock(false)?;
    }
    p.data(true)?; // The device owns DIO during the ninth clock.
    p.half_period()?;
    p.clock(true)?;
    p.half_period()?;
    let acknowledged = !p.read_data()?;
    p.clock(false)?;
    p.half_period()?;
    Ok(acknowledged)
}
