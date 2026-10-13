use super::*;
use crate::durable_host::owner::Owner;
use conduit_body::ResidentPlot;
use std::os::unix::net::UnixListener;

#[test]
fn empty_and_partial_clients_leave_same_owner_available_for_valid_read() {
    let checked = crate::plot_source::parse(
        "plot hello {\n show: presentation/text\n \"Hello.\" >> show\n}.",
    )
    .unwrap()
    .expand_entry_for_authoring()
    .unwrap();
    let resident = ResidentPlot::new(
        checked.expanded.source_document_id,
        checked.expanded.checked_plot_id,
    );
    let owner = Owner::open(StdHost::new(), resident, None, "Retained owner").unwrap();
    let mut runtime = DurableHostRuntime::new("test".into(), "test".into(), StdHost::new());
    runtime.host = HostSource::Body {
        owner: Box::new(owner),
        root: PathBuf::new(),
        running: None,
    };
    let before = runtime.owned_body_local_face().unwrap().0;
    let root = std::env::temp_dir().join(format!(
        "conduit-control-prefix-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let socket = root.join("control.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let token = [11; 32];
    let expected = before.clone();
    let worker = std::thread::spawn(move || {
        for bytes in [vec![], vec![0, 0]] {
            let mut client = UnixStream::connect(&socket).unwrap();
            client.write_all(&bytes).unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            client
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut refused = Vec::new();
            client.read_to_end(&mut refused).unwrap();
        }
        let mut client = UnixStream::connect(&socket).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        write_frame(
            &mut client,
            &Request::BodyLocalFace {
                protocol: PROTOCOL,
                token: token.to_vec(),
            },
        )
        .unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        match read_frame::<_, Response>(&mut client).unwrap() {
            Response::BodyLocalFace { presentation, .. } => assert_eq!(*presentation, expected),
            _ => panic!("same service must still answer the actual read-only owner Face request"),
        }
    });
    // Same listener, runtime and production client dispatch after both malformed clients.
    for _ in 0..3 {
        let (mut stream, _) = listener.accept().unwrap();
        serve_client(&root, &mut stream, &mut runtime, &token).unwrap();
        assert_eq!(runtime.owned_body_local_face().unwrap().0, before);
    }
    worker.join().unwrap();
    drop(listener);
    fs::remove_dir_all(root).unwrap();
}
