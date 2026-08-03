#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(target_arch = "arm")]
mod pico_w_network;

#[cfg(target_arch = "arm")]
#[cortex_m_rt::entry]
fn main() -> ! {
    use panic_halt as _;

    let peripherals = embassy_rp::init(Default::default());
    pico_w_network::run(peripherals)
}

#[cfg(not(target_arch = "arm"))]
fn main() {}
