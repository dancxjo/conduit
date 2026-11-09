//! Checked physical construction. Neither labels nor pin values grant IO.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    KindConfigurationField, KindConfigurationRule, PackageCatalogContribution,
};
use std::collections::BTreeMap;
pub const UART: &str = "esp32/uart0@1";
pub const DHT11: &str = "esp32/dht11@1";
pub const TM1637: &str = "esp32/tm1637@1";
pub const RENDERER: &str = "esp32/seven-segment-renderer@1";
pub const RECORDING: &str = "esp32/nor-record-append@1";
pub use conduit_mcu_source_build::recording::CheckedRecordingConstruction;
pub const CLOCK: &str = "esp32/monotonic-clock@1";
fn count(key: &str, minimum: u64, maximum: u64) -> KindConfigurationField {
    KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(minimum),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    }
}
fn contract(fields: Vec<KindConfigurationField>) -> BaseInitializationContract {
    BaseInitializationContract { fields }
}
pub fn uart_contract() -> BaseInitializationContract {
    contract(vec![
        count("tx", 1, 1),
        count("rx", 3, 3),
        count("baud", 115200, 115200),
    ])
}
pub fn clock_contract() -> BaseInitializationContract {
    contract(vec![
        count("maximum_duration_ms", 1, 60000),
        count("slots", 1, 4),
    ])
}
pub fn sensor_contract() -> BaseInitializationContract {
    contract(vec![
        count("data", 0, 33),
        count("start_low_us", 18000, 30000),
        count("maximum_transaction_us", 30000, 100000),
        count("maximum_interrupt_exclusion_us", 1, 20000),
    ])
}
pub fn display_contract() -> BaseInitializationContract {
    contract(vec![
        count("clk", 0, 33),
        count("dio", 0, 33),
        count("brightness", 0, 7),
        count("half_period_us", 5, 1000),
        count("settled_us", 0, 100000),
        count("maximum_clock_rise_us", 1, 10000),
        count("maximum_transaction_us", 1, 1000000),
        KindConfigurationField {
            key: "pull".into(),
            default_value: ConfigurationValue::Text("none".into()),
            rule: KindConfigurationRule::TextOneOf {
                values: vec!["none".into(), "up".into()],
            },
        },
    ])
}
pub(super) fn catalog() -> PackageCatalogContribution {
    PackageCatalogContribution {
        initialization_contracts: BTreeMap::from([
            (UART.into(), uart_contract()),
            (CLOCK.into(), clock_contract()),
            (
                RECORDING.into(),
                conduit_mcu_source_build::recording::initialization_contract(),
            ),
            (DHT11.into(), sensor_contract()),
            (TM1637.into(), display_contract()),
            (
                RENDERER.into(),
                conduit_mcu_source_build::renderer::initialization_contract(),
            ),
        ]),
        ..Default::default()
    }
}
pub(super) fn offers() -> impl Iterator<Item = conduit_host_make::ImplementationOffer> {
    [
        ("serial/text", UART),
        ("sensor/environment", DHT11),
        ("display/segmented", TM1637),
        ("clock/monotonic", CLOCK),
        (conduit_mcu_source_build::recording::BASE_KIND, RECORDING),
        ("face/renderer", RENDERER),
    ]
    .into_iter()
    .map(|(kind, implementation)| {
        let mut offer = super::offer(kind, implementation, "native-host");
        offer.target_patterns = vec!["esp32/xtensa-lx6/hw-463-esp-wroom-32".into()];
        offer
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPhysicalConstruction {
    pub maximum_wait_ms: u32,
    pub wait_slots: u8,
    pub sensor_data: u8,
    pub display_clock: u8,
    pub display_data: u8,
    pub start_low_us: u32,
    pub maximum_capture_transaction_us: u32,
    pub maximum_interrupt_exclusion_us: u32,
    pub brightness: u8,
    pub half_period_us: u32,
    pub settled_us: u32,
    pub maximum_clock_rise_us: u32,
    pub maximum_display_transaction_us: u32,
    pub weak_pullups: bool,
}
fn selected(
    checked: &CheckedHostConfiguration,
    kind: &str,
    implementation: &str,
    contract: BaseInitializationContract,
) -> Result<BTreeMap<String, ConfigurationValue>, String> {
    let mut bases = checked
        .configuration()
        .bases
        .iter()
        .filter(|b| b.kind == kind);
    let base = bases.next().ok_or_else(|| format!("missing {kind}"))?;
    if bases.next().is_some()
        || !checked
            .resolved_bases()
            .iter()
            .any(|(k, i)| k == kind && i == implementation)
    {
        return Err(format!("requires one exact {implementation}"));
    }
    base.initialization
        .as_ref()
        .ok_or_else(|| format!("missing {kind} initialization"))?
        .checked_values(&contract)
}
fn value(values: &BTreeMap<String, ConfigurationValue>, key: &str) -> Result<u32, String> {
    match values.get(key) {
        Some(ConfigurationValue::U64(n)) => (*n).try_into().map_err(|_| format!("{key} overflows")),
        _ => Err(format!("missing count {key}")),
    }
}
impl CheckedPhysicalConstruction {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        if checked.configuration().target.machine != "hw-463-esp-wroom-32" {
            return Err("native physical adapter requires the checked WROOM specimen".into());
        }
        let descriptor = crate::blue_descriptor::blue_wroom32_native();
        let binding = crate::esp32_descriptor_binding(&descriptor)
            .map_err(|e| format!("native descriptor: {e:?}"))?;
        if checked.configuration().target.make_descriptor.as_deref() != Some(binding.as_str()) {
            return Err(
                "native physical construction requires its own exact board descriptor".into(),
            );
        }
        let dram = descriptor
            .memory_regions
            .iter()
            .map(|r| r.usable_bytes)
            .sum::<u64>();
        if checked.configuration().limits.static_memory_bytes > dram {
            return Err("native static memory exceeds actual board construction extent".into());
        }
        let _uart = selected(checked, "serial/text", UART, uart_contract())?;
        let clock = selected(checked, "clock/monotonic", CLOCK, clock_contract())?;
        let wait_slots = value(&clock, "slots")?;
        if wait_slots > checked.configuration().limits.timer_slots {
            return Err("clock slots exceed checked timer construction".into());
        }
        let sensor = selected(checked, "sensor/environment", DHT11, sensor_contract())?;
        let display = selected(checked, "display/segmented", TM1637, display_contract())?;
        let data = value(&sensor, "data")?;
        let clk = value(&display, "clk")?;
        let dio = value(&display, "dio")?;
        // This board adapter currently owns only the inspected physical routes.
        // Do not advertise an arbitrary GPIO as initialized equipment.
        if (data, clk, dio) != (27, 18, 19) {
            return Err("unsupported physical GPIO route or ownership collision".into());
        }
        let start = value(&sensor, "start_low_us")?;
        let capture = value(&sensor, "maximum_transaction_us")?;
        let exclusion = value(&sensor, "maximum_interrupt_exclusion_us")?;
        let half = value(&display, "half_period_us")?;
        let settled = value(&display, "settled_us")?;
        let rise = value(&display, "maximum_clock_rise_us")?;
        let write = value(&display, "maximum_transaction_us")?;
        if capture <= start || exclusion > capture - start || write <= half + settled + rise {
            return Err("transaction ceiling cannot contain configured electrical timing".into());
        }
        Ok(Self {
            maximum_wait_ms: value(&clock, "maximum_duration_ms")?,
            wait_slots: wait_slots as u8,
            sensor_data: data as u8,
            display_clock: clk as u8,
            display_data: dio as u8,
            start_low_us: start,
            maximum_capture_transaction_us: capture,
            maximum_interrupt_exclusion_us: exclusion,
            brightness: value(&display, "brightness")? as u8,
            half_period_us: half,
            settled_us: settled,
            maximum_clock_rise_us: rise,
            maximum_display_transaction_us: write,
            weak_pullups: matches!(display.get("pull"),Some(ConfigurationValue::Text(p)) if p=="up"),
        })
    }
}
