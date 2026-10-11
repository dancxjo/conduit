//! Bounded hardware-counter spacing for an admitted protocol clock poll.

pub fn space_protocol_clock_poll() -> Result<(), &'static str> {
    let frequency = super::counter_frequency();
    let ticks = frequency
        .checked_add(999)
        .map(|value| value / 1000)
        .filter(|value| *value != 0)
        .ok_or("protocol-clock-spacing-unavailable")?;
    let start = super::read_counter();
    for _ in 0..1_000_000 {
        let elapsed = super::read_counter()
            .checked_sub(start)
            .ok_or("protocol-clock-spacing-lost")?;
        if elapsed >= ticks {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err("protocol-clock-spacing-no-progress")
}
