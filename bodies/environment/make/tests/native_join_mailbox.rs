//! Production join HTTP mailbox with real strict decoder and unavailable Store.
//! Hardware Root entrances panic; this proves transport custody, not admission.
// Embedded profile flags do not exist in this CPU fixture crate.
#![allow(unexpected_cfgs)]
extern crate alloc;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
#[allow(dead_code)] // Compatibility byte-slice API is covered by native_restart tests.
mod native_body_checkpoint;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_join.rs"]
mod native_body_join;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_join_http.rs"]
mod native_body_join_http;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_joined_host_wire.rs"]
mod native_joined_host_wire;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart.rs"]
#[allow(dead_code)] // Legacy envelope helpers are covered by native_restart tests.
mod native_restart;
mod native_evidence_region {
    use crate::native_body_checkpoint::{Refusal, Storage};
    use std::sync::{Arc, Mutex};
    #[derive(Clone)]
    pub struct BodyRegion(pub Arc<Mutex<(Vec<u8>, bool)>>);
    pub fn test_empty() -> BodyRegion {
        BodyRegion(Arc::new(Mutex::new((vec![255; 65536], true))))
    }
    impl Storage for BodyRegion {
        fn capacity(&self) -> u32 {
            65536
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
            out.copy_from_slice(&self.0.lock().unwrap().0[at as usize..at as usize + out.len()]);
            Ok(())
        }
        fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
            let mut state = self.0.lock().unwrap();
            if !state.1 {
                return Err(Refusal::Storage);
            }
            for (old, new) in state.0[at as usize..at as usize + bytes.len()]
                .iter_mut()
                .zip(bytes)
            {
                assert_eq!(*old & new, *new);
                *old &= new;
            }
            Ok(())
        }
        fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal> {
            let mut state = self.0.lock().unwrap();
            if !state.1 {
                return Err(Refusal::Storage);
            }
            state.0[usize::from(bank) * 32768..(usize::from(bank) + 1) * 32768].fill(255);
            Ok(())
        }
    }
}

#[path = "native_flash_providers/manual_clock.rs"]
mod manual_clock;
mod native_body_join_uart {
    pub fn entropy(_: &mut [u8; 32]) -> Result<(), &'static str> {
        Err("CPU entropy unavailable")
    }
}
// Only hardware Root entrances are replaced. None may execute while the Store
// is unavailable; the production mailbox, decoder, cache and Store are real.
mod native_root {
    pub struct NativeRoot;
    impl NativeRoot {
        pub fn body_join_scope(&self) -> Result<(), &'static str> {
            Ok(())
        }
        pub fn begin_body_observer_return(
            &mut self,
            _: &crate::native_body_join::return_wire::Begin,
            _: [u8; 32],
            _: u64,
        ) -> Result<crate::native_body_join::returning::Challenge, &'static str> {
            panic!("unexpected admission")
        }
        pub fn complete_body_observer_return(
            &mut self,
            _: &crate::native_body_join::return_wire::Complete,
            _: u64,
        ) -> Result<crate::native_body_join::recovery::Receipt, &'static str> {
            panic!("unexpected admission")
        }
        pub fn admit_body_observer_request(
            &mut self,
            _: &conduit_body::PortableSpawnAdmissionRequest,
            _: u64,
        ) -> Result<crate::native_joined_host_wire::JoinedHostReceipt, &'static str> {
            panic!("unexpected admission")
        }
        pub fn admit_body_membership_request(
            &mut self,
            _: &conduit_body::PortableMembershipAdmissionRequest,
            _: u64,
        ) -> Result<conduit_body::PortableMembershipAdmissionReceipt, &'static str> {
            panic!("unexpected admission")
        }
        pub fn recover_body_observer(
            &self,
            _: &crate::native_body_join::recovery::Request,
        ) -> Result<crate::native_body_join::recovery::Receipt, &'static str> {
            panic!("unexpected admission")
        }
    }
}

#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
