//! Finite owner-local arm; no process-global permission switch.
#[derive(Default)]
pub(super) struct State {
    pending: Option<u8>,
}
#[derive(Debug)]
pub(super) enum Error {
    Busy,
    Row,
}
impl State {
    pub(super) fn arm(&mut self, row: usize) -> Result<(), Error> {
        if row >= 18 {
            return Err(Error::Row);
        }
        if self.pending.is_some() {
            return Err(Error::Busy);
        }
        self.pending = Some(row as u8);
        Ok(())
    }
    pub(super) fn take(&mut self) -> Option<u8> {
        self.pending.take()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exactly_one_matching_owned_entry_consumes_the_finite_arm() {
        let mut a = State::default();
        let mut b = State::default();
        assert!(a.arm(18).is_err());
        assert_eq!(a.take(), None);
        a.arm(0).unwrap();
        assert!(a.arm(1).is_err());
        assert_eq!(b.take(), None);
        assert_eq!(a.take(), Some(0));
        assert_eq!(a.take(), None);
        a.arm(17).unwrap();
        assert_eq!(a.take(), Some(17));
    }
}
