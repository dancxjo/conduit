#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{FinanceCurrency, FinanceCurrencyPair, FinanceMoneyComparison};

mod catalog;
mod finance;
mod reference;

pub use catalog::*;
pub use finance::*;
pub use reference::*;
