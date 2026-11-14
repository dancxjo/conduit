//! Actual generated Source owners; opaque loans are bound only after admission.
#[allow(dead_code)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/resident.rs"));
}
pub use generated::*;
// SAFETY: the generated opaque tickets retain the exact exclusively preflighted
// node and disjoint initialized entry. Back state consists solely of static
// schemas and inline buffers, funded by the linked physical image reservation.
unsafe impl<'a> conduit_mcu_host_runtime::execution_factory::PreparedResidentEntry
    for generated::PreparedResidentEntryLoan<'a>
{
    type Back = generated::BorrowedBack<'a>;
    fn node(&self) -> u16 {
        self.node()
    }
    fn terminal_transductions<const P: usize>(
        &self,
    ) -> Option<[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]> {
        Some(generated::PreparedResidentEntryLoan::terminal_transductions::<P>(self))
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    fn bind(self) -> (u16, Self::Back) {
        self.bind()
    }
}

pub fn inventory() -> Option<generated::Initialized> {
    generated::Initialized::prepare().ok()
}
