//! Sparse optional metadata has no inline cost on alloc-enabled local kernels.
//! No allocation occurs while stepping; non-alloc profiles retain fixed backing.
use super::AssignedTerminalTransduction;
use core::ops::Index;
type Contracts<const N: usize, const P: usize> = [[Option<AssignedTerminalTransduction>; P]; N];
#[cfg(feature = "alloc")]
pub(super) struct TerminalContracts<const N: usize, const P: usize>(
    Option<alloc::boxed::Box<Contracts<N, P>>>,
);
#[cfg(not(feature = "alloc"))]
pub(super) struct TerminalContracts<const N: usize, const P: usize>(Contracts<N, P>);
impl<const N: usize, const P: usize> TerminalContracts<N, P> {
    pub const fn empty() -> Self {
        #[cfg(feature = "alloc")]
        {
            Self(None)
        }
        #[cfg(not(feature = "alloc"))]
        {
            Self([[None; P]; N])
        }
    }
    pub fn preparation_bytes(contracts: &Contracts<N, P>) -> usize {
        #[cfg(feature = "alloc")]
        {
            if contracts.iter().flatten().any(Option::is_some) {
                core::mem::size_of::<Contracts<N, P>>()
            } else {
                0
            }
        }
        #[cfg(not(feature = "alloc"))]
        {
            let _ = contracts;
            0
        }
    }
    pub fn prepare(contracts: Contracts<N, P>) -> Self {
        #[cfg(feature = "alloc")]
        {
            if Self::preparation_bytes(&contracts) == 0 {
                Self(None)
            } else {
                Self(Some(alloc::boxed::Box::new(contracts)))
            }
        }
        #[cfg(not(feature = "alloc"))]
        {
            Self(contracts)
        }
    }
}
impl<const N: usize, const P: usize> Index<usize> for TerminalContracts<N, P> {
    type Output = [Option<AssignedTerminalTransduction>; P];
    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < N);
        #[cfg(feature = "alloc")]
        {
            match &self.0 {
                Some(contracts) => &contracts[index],
                None => &[None; P],
            }
        }
        #[cfg(not(feature = "alloc"))]
        {
            &self.0[index]
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_table_reserves_no_heap_and_keeps_exact_none_ports() {
        let empty = TerminalContracts::<15, 4>::empty();
        assert_eq!(
            TerminalContracts::<15, 4>::preparation_bytes(&[[None; 4]; 15]),
            0
        );
        for n in 0..15 {
            assert_eq!(empty[n], [None; 4]);
        }
        #[cfg(feature = "alloc")]
        assert_eq!(
            core::mem::size_of_val(&empty),
            core::mem::size_of::<Option<alloc::boxed::Box<Contracts<15, 4>>>>()
        );
    }
}
