//! Allocation-independent refusal for canonical slot transactions.
use crate::StorageError;
#[derive(Debug, PartialEq, Eq)]
pub enum EncodedValueRefusal<E> {
    Storage(StorageError),
    Encoder(E),
    Length,
}
