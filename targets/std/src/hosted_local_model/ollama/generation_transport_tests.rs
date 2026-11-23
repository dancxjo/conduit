#[cfg(unix)]
mod unix {
    use super::super::{test_support::*, *};
    use std::{thread, time::Instant};

    #[test]
    fn cancelled_before_dispatch_never_contacts_provider() {
        let server = Server::start([]);
        let transport = Transport::prepare().unwrap();
        transport.cancellation.cancel();
        assert_eq!(
            transport.generate(&server.endpoint, b"{}"),
            Err(LocalModelAdapterTerminal::Cancelled)
        );
        assert!(server.requests.try_recv().is_err());
    }

    #[test]
    fn stalled_response_cancellation_reaps_before_return_and_discards_bytes() {
        let server = Server::start([Response::Hold(LATE)]);
        let transport = Transport::prepare().unwrap();
        let cancellation = transport.cancellation();
        let endpoint = server.endpoint.clone();
        let worker = thread::spawn(move || transport.generate(&endpoint, b"{}"));
        server.request();
        let started = Instant::now();
        cancellation.cancel();
        assert_eq!(
            worker.join().unwrap(),
            Err(LocalModelAdapterTerminal::Cancelled)
        );
        server.closed.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
        println!(
            "cancel-to-local-release-ms={}",
            started.elapsed().as_millis()
        );
    }

    #[test]
    fn timeout_loss_and_success_have_distinct_transport_dispositions() {
        let server = Server::start([Response::Hold(LATE), Response::Lost, Response::Json(FRESH)]);
        let mut transport = Transport::prepare().unwrap();
        transport.timeout = Duration::from_millis(40);
        assert_eq!(
            transport.generate(&server.endpoint, b"{}"),
            Err(LocalModelAdapterTerminal::TimedOut)
        );
        transport.timeout = Duration::from_secs(2);
        assert_eq!(
            transport.generate(&server.endpoint, b"{}"),
            Err(LocalModelAdapterTerminal::ProviderLost)
        );
        assert_eq!(
            transport.generate(&server.endpoint, b"{}"),
            Ok(FRESH.as_bytes().to_vec())
        );
    }

    #[test]
    fn old_cancellation_token_cannot_abort_a_fresh_transport() {
        let old = Transport::prepare().unwrap();
        let old_token = old.cancellation();
        drop(old);
        let replacement = Transport::prepare().unwrap();
        let server = Server::start([Response::Hold(FRESH)]);
        let endpoint = server.endpoint.clone();
        let running = thread::spawn(move || replacement.generate(&endpoint, b"{}"));
        server.request();
        old_token.cancel();
        server.send_late();
        assert_eq!(running.join().unwrap(), Ok(FRESH.as_bytes().to_vec()));
    }

    #[test]
    fn oversized_response_is_not_parsed_as_a_retained_success_prefix() {
        let server = Server::start([Response::Bytes(vec![b'x'; MAXIMUM_RESPONSE_BYTES + 1])]);
        let transport = Transport::prepare().unwrap();
        let result = transport.generate(&server.endpoint, b"{}");
        assert!(
            matches!(
                result,
                Err(LocalModelAdapterTerminal::ResponseBoundExceeded)
            ),
            "expected wire capacity refusal, observed terminal {:?}",
            result.as_ref().err()
        );
    }

    #[test]
    fn model_wire_metadata_above_the_ordinary_process_cap_is_preserved() {
        let response = serde_json::to_vec(&serde_json::json!({
            "response": "text", "done": true, "context": vec![0; 70_000],
            "prompt_eval_count": 1, "eval_count": 1,
        }))
        .unwrap();
        assert!(response.len() > 131_116);
        let server = Server::start([Response::Bytes(response.clone())]);
        let transport = Transport::prepare().unwrap();
        assert_eq!(MAXIMUM_RESPONSE_BYTES, 16 * 1024 * 1024);
        let retained = transport.generate(&server.endpoint, b"{}").unwrap();
        assert!(
            retained == response,
            "valid model wire metadata was changed or truncated"
        );
    }

    #[test]
    fn buffered_cleanup_overflow_cannot_rewrite_an_established_terminal() {
        use crate::hosted_process::CapturedOutput;
        use std::os::unix::process::ExitStatusExt;
        for (terminal, expected, bound_retired) in [
            (
                ProcessTerminal::TimedOut,
                LocalModelAdapterTerminal::TimedOut,
                false,
            ),
            (
                ProcessTerminal::Cancelled,
                LocalModelAdapterTerminal::Cancelled,
                false,
            ),
            (
                ProcessTerminal::Exited(std::process::ExitStatus::from_raw(28 << 8)),
                LocalModelAdapterTerminal::TimedOut,
                false,
            ),
            (
                ProcessTerminal::ProviderLost("earlier pipe loss".into()),
                LocalModelAdapterTerminal::ProviderLost,
                false,
            ),
            (
                ProcessTerminal::ProviderLost("explicit bound retirement".into()),
                LocalModelAdapterTerminal::ResponseBoundExceeded,
                true,
            ),
            (
                ProcessTerminal::Exited(std::process::ExitStatus::from_raw(0)),
                LocalModelAdapterTerminal::ResponseBoundExceeded,
                false,
            ),
        ] {
            let report = ProcessReport {
                terminal,
                stdout: CapturedOutput {
                    retained: Vec::new(),
                    observed_bytes: MAXIMUM_RESPONSE_BYTES as u64 + 1,
                },
                stderr: CapturedOutput::default(),
                elapsed_millis: 1,
                launched: true,
                stdout_bound_retired: bound_retired,
            };
            assert_eq!(finish(report), Err(expected));
        }
    }
}
