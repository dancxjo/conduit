#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(target_arch = "arm")]
mod firmware {
    use conduit_embedded::{
        HIL_PROTOCOL_VERSION, HilEventFrame, HilRequest, HilRunHeader, HilRunStatus, RunControl,
        RunIdentity, RunStatus, execute_static_plan,
    };
    use conduit_rp2040_hil::{
        FIRMWARE_IDENTITY, PLAN_HASH, ReferenceHost, ReferenceStorage, drivers, plan, profile,
        with_capability_report,
    };
    use panic_halt as _;
    use rand_core::RngCore;
    use rp_pico::entry;
    use rp_pico::hal;
    use rp_pico::hal::pac;
    use usb_device::{class_prelude::UsbBusAllocator, prelude::*};
    use usbd_serial::SerialPort;

    const MAXIMUM_USB_WRITE_POLLS: u32 = 100_000;

    #[entry]
    fn main() -> ! {
        let mut pac = pac::Peripherals::take().expect("single peripheral init");
        let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
        let clocks = hal::clocks::init_clocks_and_plls(
            rp_pico::XOSC_CRYSTAL_FREQ,
            pac.XOSC,
            pac.CLOCKS,
            pac.PLL_SYS,
            pac.PLL_USB,
            &mut pac.RESETS,
            &mut watchdog,
        )
        .ok()
        .expect("RP2040 clocks");
        let mut random = hal::rosc::RingOscillator::new(pac.ROSC).initialize();
        let mut boot_id = [0; 16];
        random.fill_bytes(&mut boot_id);
        let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
            pac.USBCTRL_REGS,
            pac.USBCTRL_DPRAM,
            clocks.usb_clock,
            true,
            &mut pac.RESETS,
        ));
        let mut serial = SerialPort::new(&usb_bus);
        let mut usb_device = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x1209, 0xC028))
            .strings(&[StringDescriptors::default()
                .manufacturer("Conduit")
                .product("RP2040 HIL")
                .serial_number("conduit-rp2040-hil")])
            .expect("static USB strings")
            .device_class(2)
            .build();
        let storage = cortex_m::singleton!(: ReferenceStorage = ReferenceStorage::new())
            .expect("single firmware init");
        let profile = profile();
        let plan = plan(&profile);
        let mut request_bytes = [0; HilRequest::ENCODED_BYTES];
        let mut request_length = 0;
        let mut run_sequence = 0_u64;
        loop {
            if !usb_device.poll(&mut [&mut serial]) {
                continue;
            }
            let mut packet = [0; 64];
            let Ok(count) = serial.read(&mut packet) else {
                continue;
            };
            let remaining = request_bytes.len() - request_length;
            let copied = count.min(remaining);
            request_bytes[request_length..request_length + copied]
                .copy_from_slice(&packet[..copied]);
            request_length += copied;
            if request_length != request_bytes.len() {
                continue;
            }
            request_length = 0;
            let Ok(request) = HilRequest::decode(&request_bytes) else {
                continue;
            };
            run_sequence = run_sequence.wrapping_add(1);
            let run = RunIdentity {
                boot_id,
                run_sequence,
            };
            let capability_report_hash =
                with_capability_report(run_sequence, |report| report.identity);
            let mut drivers = drivers();
            let mut host = ReferenceHost { indicator: false };
            let result = if request.expected_plan_hash == PLAN_HASH {
                execute_static_plan(
                    &plan,
                    &profile,
                    storage,
                    &mut drivers,
                    &mut host,
                    run,
                    RunControl {
                        maximum_decisions: request.maximum_decisions,
                        cancellation_at_decision: None,
                        initial_tick: 0,
                    },
                )
            } else {
                Err(conduit_embedded::EmbeddedError::InvalidStaticPlan)
            };
            let (status, decisions, evidence_records) = match result {
                Ok(summary) => (
                    match summary.status {
                        RunStatus::Succeeded => HilRunStatus::Succeeded,
                        RunStatus::Cancelled => HilRunStatus::Cancelled,
                    },
                    summary.decisions,
                    summary.evidence_records,
                ),
                Err(_) => (HilRunStatus::Failed, 0, 0),
            };
            let header = HilRunHeader {
                protocol_version: HIL_PROTOCOL_VERSION,
                nonce: request.nonce,
                plan_hash: plan.full_plan_hash,
                firmware_identity: FIRMWARE_IDENTITY,
                capability_report_hash,
                run,
                status,
                decisions,
                evidence_records,
            };
            let mut header_bytes = [0; HilRunHeader::ENCODED_BYTES];
            header.encode(&mut header_bytes);
            if !write_all(&mut usb_device, &mut serial, &header_bytes) {
                continue;
            }
            if status != HilRunStatus::Failed {
                for event in storage.events() {
                    let mut frame_bytes = [0; HilEventFrame::ENCODED_BYTES];
                    HilEventFrame {
                        nonce: request.nonce,
                        event: *event,
                    }
                    .encode(&mut frame_bytes)
                    .expect("fixed representative value");
                    if !write_all(&mut usb_device, &mut serial, &frame_bytes) {
                        break;
                    }
                }
            }
            core::hint::black_box(host.indicator);
        }
    }

    fn write_all<B: usb_device::bus::UsbBus>(
        device: &mut UsbDevice<'_, B>,
        serial: &mut SerialPort<'_, B>,
        mut bytes: &[u8],
    ) -> bool {
        for _ in 0..MAXIMUM_USB_WRITE_POLLS {
            if bytes.is_empty() {
                return true;
            }
            let _ = device.poll(&mut [serial]);
            if let Ok(written) = serial.write(bytes) {
                bytes = &bytes[written..];
            }
        }
        false
    }
}

#[cfg(not(target_arch = "arm"))]
fn main() {}
