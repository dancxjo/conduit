//! A listener may briefly close between native HTTP connections. Retry only
//! transient socket errors, keeping the exact query and original finite budget.
use crate::Result;
use std::{
    io::ErrorKind,
    time::{Duration, Instant},
};

pub(super) fn exchange<T>(
    deadline: Instant,
    attempts: &mut u8,
    mut operation: impl FnMut() -> Result<T>,
) -> Result<T> {
    loop {
        if *attempts == 0 || Instant::now() >= deadline {
            return Err("current producer transport budget exhausted".into());
        }
        *attempts -= 1;
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) => {
                let transient = error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                    matches!(
                        error.kind(),
                        ErrorKind::ConnectionRefused
                            | ErrorKind::ConnectionReset
                            | ErrorKind::ConnectionAborted
                            | ErrorKind::UnexpectedEof
                            | ErrorKind::TimedOut
                    )
                });
                if !transient || *attempts == 0 {
                    return Err(error);
                }
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .ok_or("current producer transport deadline")?;
                std::thread::sleep(remaining.min(Duration::from_millis(100)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn listener_rearming_reuses_query_and_shared_attempt_budget() {
        let payload = [41; 64];
        let mut attempts = 4;
        let mut calls = 0;
        let value = exchange(
            Instant::now() + Duration::from_secs(2),
            &mut attempts,
            || {
                calls += 1;
                assert_eq!(payload, [41; 64]);
                if calls < 3 {
                    Err(std::io::Error::from(ErrorKind::ConnectionRefused).into())
                } else {
                    Ok(7)
                }
            },
        )
        .unwrap();
        assert_eq!((value, calls, attempts), (7, 3, 1));
        exchange(
            Instant::now() + Duration::from_secs(2),
            &mut attempts,
            || Ok(()),
        )
        .unwrap();
        assert_eq!(attempts, 0);
        assert!(exchange(
            Instant::now() + Duration::from_secs(2),
            &mut attempts,
            || -> Result<()> { panic!("spent budget cannot call transport") }
        )
        .is_err());
    }
    #[test]
    fn protocol_refusal_and_expired_deadline_do_not_retry() {
        let mut attempts = 16;
        let mut calls = 0;
        assert!(exchange(
            Instant::now() + Duration::from_secs(2),
            &mut attempts,
            || -> Result<()> {
                calls += 1;
                Err("invalid receipt".into())
            }
        )
        .is_err());
        assert_eq!((calls, attempts), (1, 15));
        assert!(exchange(Instant::now(), &mut attempts, || -> Result<()> {
            panic!("expired deadline cannot call transport")
        })
        .is_err());
        assert_eq!(attempts, 15);
    }
}
