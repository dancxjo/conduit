#![cfg(all(target_os = "linux", feature = "carrier-source"))]
//! Confined native process against bounded loopback devices, not physical proof.
use conduit_core::{BootId, HostId, OfferGeneration, ResourceGenerationId};
use conduit_environment_host::source_operator::*;
use conduit_host_make::*;
use conduit_std_host::isolated_http_base::IsolatedHttpBaseConfig;
use conduit_std_host::{StdHost, StdHostConfig};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
fn checked() -> CheckedHostConfiguration {
    let packages = MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    let source =
        include_str!("../../../../targets/rp2040/profiles/pico-environment-usb.host.conduit");
    check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap()
}
fn ready<F: std::future::Future>(future: F) -> F::Output {
    struct Noop;
    impl std::task::Wake for Noop {
        fn wake(self: Arc<Self>) {}
    }
    let waker = std::task::Waker::from(Arc::new(Noop));
    let mut cx = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(v) => v,
        std::task::Poll::Pending => panic!("native bounded fixture unexpectedly suspended"),
    }
}
#[test]
fn two_exact_devices_commit_on_one_actual_host_clock_without_fabricated_membership() {
    let checked = checked();
    let identity = checked.configuration_id().to_owned();
    let finished = Arc::new(AtomicBool::new(false));
    let mut workers = Vec::new();
    let mut configurations = Vec::new();
    for device in 0..2 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = listener.local_addr().unwrap();
        let stop = finished.clone();
        let id = identity.clone();
        workers.push(std::thread::spawn(move||{let mut counter=0;while !stop.load(Ordering::SeqCst) {let (mut stream,_)=match listener.accept(){Ok(v)=>v,Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{std::thread::sleep(Duration::from_millis(5));continue},Err(e)=>panic!("{e}")};stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();let mut request=Vec::new();while !request.ends_with(b"\r\n\r\n"){let mut b=[0];stream.read_exact(&mut b).unwrap();request.push(b[0]);assert!(request.len()<4096);}assert!(request.starts_with(b"GET /environment HTTP/1.1\r\n"));counter+=1;let body=serde_json::json!({"role":"conduit.environment-wifi-carrier@1","build":"01".repeat(32),"configuration":id,"body_member":false,"samples":counter,"attempts":counter,"sensor_error":false,"temperature_centi_celsius":2000+device,"pressure_centi_pascal":10000000,"humidity_percent_q10":null,"sampled_at_boot_ms":123+device}).to_string();write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();}counter}));
        configurations.push(NetworkSourceConfiguration {
            http: IsolatedHttpBaseConfig {
                executable: env!("CARGO_BIN_EXE_conduit-environment-http-provider").into(),
                base_instance_id: format!("fixture/http/{device}").into(),
                provider_generation: 1,
                authority: format!("device{device}.invalid:{}", endpoint.port()),
                endpoint,
                resource_generation_id: ResourceGenerationId(format!(
                    "fixture/http/resource/{device}"
                )),
            },
            firmware: [1; 32],
        });
    }
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("fixture/single-collector"),
        boot_id: BootId::from("fixture/single-collector-boot"),
        offer_generation: OfferGeneration(1),
    });
    let mut collector = CollectorRoot::new(host);
    let mut wrong = configurations[0].clone();
    wrong.firmware = [2; 32];
    assert!(matches!(
        collector.attach_network(wrong, &checked, Duration::from_secs(31)),
        Err(Refusal::Acquisition)
    ));
    let a = collector
        .attach_network(configurations.remove(0), &checked, Duration::from_secs(31))
        .unwrap();
    let b = collector
        .attach_network(configurations.remove(0), &checked, Duration::from_secs(31))
        .unwrap();
    let inventory = collector.advertisement().unwrap();
    let sources = inventory
        .capabilities
        .iter()
        .filter(|cap| {
            cap.kind_id.as_str() == conduit_environment_runtime::environmental_source::HOST_CALL
        })
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    assert_ne!(sources[0].capability_id, sources[1].capability_id);
    let bases = inventory
        .bases
        .iter()
        .filter(|base| {
            base.mechanism_family.as_str()
                == conduit_environment_runtime::environmental_source::RESOURCE_CLASS
        })
        .collect::<Vec<_>>();
    assert_eq!(bases.len(), 2);
    assert!(bases
        .iter()
        .all(|base| base.capability_ids.len() == 3 && base.resource_pool_ids.len() == 2));
    let before = boot_ms();
    let x = ready(collector.sample(a)).unwrap();
    let y = ready(collector.sample(b)).unwrap();
    let z = ready(collector.sample(a)).unwrap();
    assert_eq!(collector.advertisement().unwrap(), inventory);
    assert_eq!(x.scope().host(), y.scope().host());
    assert_eq!(x.scope().boot(), y.scope().boot());
    assert_ne!(x.session(), y.session());
    assert_eq!(x.session(), z.session());
    assert_eq!((x.sequence(), y.sequence(), z.sequence()), (1, 1, 2));
    assert!(x.body().is_none() && y.body().is_none());
    let contract=conduit_environment_runtime::environmental_reading::source_observation::ObservationContract::prepare().unwrap();
    let x = contract.decode(x.canonical()).unwrap();
    let y = contract.decode(y.canonical()).unwrap();
    assert!(x.observed_at_host_boot_ms <= y.observed_at_host_boot_ms);
    assert!(before <= x.observed_at_host_boot_ms && y.observed_at_host_boot_ms <= boot_ms());
    finished.store(true, Ordering::SeqCst);
    for worker in workers {
        assert!(worker.join().unwrap() >= 3);
    }
}

fn boot_ms() -> u64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut t) },
        0
    );
    t.tv_sec as u64 * 1000 + t.tv_nsec as u64 / 1_000_000
}
