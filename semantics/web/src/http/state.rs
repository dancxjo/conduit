//! Finite semantic correlation state shared by client/server realization tests.

use super::{HttpServerResponseRefusal, HttpTransactionId, HTTP_MAXIMUM_IN_FLIGHT};
use alloc::vec::Vec;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TransactionState {
    AwaitingResponse,
    Responded,
    Cancelled,
    ProviderLost,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Transaction {
    id: HttpTransactionId,
    state: TransactionState,
}

/// Exact bounded correlation book. Construction belongs before Play start;
/// mutations never grow beyond the admitted transaction count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpServerTransactions {
    entries: Vec<Transaction>,
}

impl HttpServerTransactions {
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(HTTP_MAXIMUM_IN_FLIGHT as usize),
        }
    }

    pub fn admit_request(
        &mut self,
        id: HttpTransactionId,
    ) -> Result<(), HttpServerResponseRefusal> {
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(HttpServerResponseRefusal::StaleTransaction);
        }
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.state != TransactionState::AwaitingResponse)
        {
            *entry = Transaction {
                id,
                state: TransactionState::AwaitingResponse,
            };
            return Ok(());
        }
        if self.entries.len() == HTTP_MAXIMUM_IN_FLIGHT as usize {
            return Err(HttpServerResponseRefusal::Capacity);
        }
        self.entries.push(Transaction {
            id,
            state: TransactionState::AwaitingResponse,
        });
        Ok(())
    }

    pub fn accept_response(
        &mut self,
        id: HttpTransactionId,
    ) -> Result<(), HttpServerResponseRefusal> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or(HttpServerResponseRefusal::UnknownTransaction)?;
        match entry.state {
            TransactionState::AwaitingResponse => {
                entry.state = TransactionState::Responded;
                Ok(())
            }
            TransactionState::Responded => Err(HttpServerResponseRefusal::DuplicateResponse),
            TransactionState::Cancelled | TransactionState::ProviderLost => {
                Err(HttpServerResponseRefusal::LateResponse)
            }
        }
    }

    pub fn cancel(&mut self, id: HttpTransactionId) -> Result<(), HttpServerResponseRefusal> {
        self.set_terminal(id, TransactionState::Cancelled)
    }

    pub fn provider_lost(
        &mut self,
        id: HttpTransactionId,
    ) -> Result<(), HttpServerResponseRefusal> {
        self.set_terminal(id, TransactionState::ProviderLost)
    }

    fn set_terminal(
        &mut self,
        id: HttpTransactionId,
        state: TransactionState,
    ) -> Result<(), HttpServerResponseRefusal> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or(HttpServerResponseRefusal::UnknownTransaction)?;
        if entry.state != TransactionState::AwaitingResponse {
            return Err(HttpServerResponseRefusal::LateResponse);
        }
        entry.state = state;
        Ok(())
    }
}

impl Default for HttpServerTransactions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correlation_pressure_duplicate_late_and_unknown_are_distinct() {
        let mut state = HttpServerTransactions::new();
        for id in 0..HTTP_MAXIMUM_IN_FLIGHT {
            state
                .admit_request(HttpTransactionId(u64::from(id)))
                .unwrap();
        }
        assert_eq!(
            state.admit_request(HttpTransactionId(99)),
            Err(HttpServerResponseRefusal::Capacity)
        );
        state.accept_response(HttpTransactionId(0)).unwrap();
        assert_eq!(
            state.accept_response(HttpTransactionId(0)),
            Err(HttpServerResponseRefusal::DuplicateResponse)
        );
        state.cancel(HttpTransactionId(1)).unwrap();
        assert_eq!(
            state.accept_response(HttpTransactionId(1)),
            Err(HttpServerResponseRefusal::LateResponse)
        );
        assert_eq!(
            state.accept_response(HttpTransactionId(100)),
            Err(HttpServerResponseRefusal::UnknownTransaction)
        );
    }

    #[test]
    fn replayed_request_identity_is_stale_not_pressure() {
        let mut state = HttpServerTransactions::new();
        state.admit_request(HttpTransactionId(7)).unwrap();
        assert_eq!(
            state.admit_request(HttpTransactionId(7)),
            Err(HttpServerResponseRefusal::StaleTransaction)
        );
    }

    #[test]
    fn terminal_slot_is_reused_without_growing_correlation_storage() {
        let mut state = HttpServerTransactions::new();
        for id in 0..HTTP_MAXIMUM_IN_FLIGHT {
            state
                .admit_request(HttpTransactionId(u64::from(id)))
                .unwrap();
        }
        state.accept_response(HttpTransactionId(0)).unwrap();
        state.admit_request(HttpTransactionId(10)).unwrap();
        assert_eq!(state.entries.len(), HTTP_MAXIMUM_IN_FLIGHT as usize);
        assert_eq!(
            state.accept_response(HttpTransactionId(0)),
            Err(HttpServerResponseRefusal::UnknownTransaction)
        );
        assert_eq!(
            state.admit_request(HttpTransactionId(11)),
            Err(HttpServerResponseRefusal::Capacity)
        );
    }
}
