//! Attended AP/HTTP bench. No native Host/Body installation is asserted.
use embassy_executor::Spawner;
use embassy_net::{Config, Ipv4Address, Ipv4Cidr, Stack, StackResources, StaticConfigV4};
use esp_radio::wifi::{ap::AccessPointConfig, Config as WifiConfig, ControllerConfig, CountryInfo};
use static_cell::StaticCell;
static AP_RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();
#[cfg(not(all(feature = "network-mdns", feature = "owner-wifi")))]
static STATION_RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();
#[cfg(all(feature = "network-mdns", feature = "owner-wifi"))]
static STATION_RESOURCES: StaticCell<StackResources<5>> = StaticCell::new();
#[cfg(all(feature = "network-mdns", feature = "owner-wifi"))]
pub const MDNS_SOCKET_BYTES: usize =
    core::mem::size_of::<StackResources<5>>() - core::mem::size_of::<StackResources<4>>();
/// Actual initialized resources retained by the composition root. This value is
/// not an offer or attachment, and makes no claim of station association.
pub struct Radio {
    pub controller: esp_radio::wifi::WifiController<'static>,
    pub stacks: (Stack<'static>, Stack<'static>),
    pub ap_address: StaticConfigV4,
    pub ap_config: AccessPointConfig,
}
#[embassy_executor::task(pool_size = 2)]
async fn runner(mut runner: embassy_net::Runner<'static, esp_radio::wifi::Interface<'static>>) {
    runner.run().await
}
pub fn start(spawner: Spawner, wifi: esp_hal::peripherals::WIFI<'static>, seed: u64) -> Radio {
    let ap = AccessPointConfig::default()
        .with_ssid("Conduit-Climate")
        .with_max_connections(2);
    let config = ControllerConfig::default()
        .with_country_info(CountryInfo::from(*b"US"))
        .with_rx_queue_size(3)
        .with_tx_queue_size(2)
        .with_static_rx_buf_num(4)
        .with_dynamic_rx_buf_num(8)
        .with_dynamic_tx_buf_num(4)
        .with_ampdu_rx_enable(false)
        .with_ampdu_tx_enable(false)
        .with_initial_config(WifiConfig::AccessPoint(ap.clone()));
    let (controller, interfaces) = esp_radio::wifi::new(wifi, config).unwrap();
    let ap_address = StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 4, 1), 24),
        gateway: None,
        dns_servers: Default::default(),
    };
    let (stack, net_runner) = embassy_net::new(
        interfaces.access_point,
        Config::ipv4_static(ap_address.clone()),
        AP_RESOURCES.init(StackResources::new()),
        seed,
    );
    spawner.spawn(runner(net_runner).unwrap());
    // No station address is configured until a selected switch operation enables
    // DHCP. Both interface runners remain owned across radio mode changes.
    let (station_stack, station_runner) = embassy_net::new(
        interfaces.station,
        Config::default(),
        STATION_RESOURCES.init(StackResources::new()),
        seed.rotate_left(32),
    );
    spawner.spawn(runner(station_runner).unwrap());
    #[cfg(not(feature = "native-root"))]
    spawner.spawn(crate::services::run((stack, station_stack), None).unwrap());
    Radio {
        controller,
        stacks: (stack, station_stack),
        ap_address,
        ap_config: ap,
    }
}
