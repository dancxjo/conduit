//! Nine independently claimed physical stores. No compositor arena duplication.
use crate::{
    protected_region::DomainRefusal,
    service_memory::{ServiceMemoryLayout, ServiceProfile},
};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};
#[repr(C, align(4096))]
#[derive(Clone, Copy)]
pub(super) struct Table(pub(super) [u64; 512]);
#[repr(C, align(4096))]
struct Bytes<const N: usize>([u8; N]);
const C: ServiceMemoryLayout = ServiceMemoryLayout::compositor();
const D: ServiceMemoryLayout = ServiceMemoryLayout::draft();
const H: ServiceMemoryLayout = ServiceMemoryLayout::home();
const R: ServiceMemoryLayout = ServiceMemoryLayout::runtime();
const A: ServiceMemoryLayout = ServiceMemoryLayout::application();
const N: ServiceMemoryLayout = ServiceMemoryLayout::network();
const O: ServiceMemoryLayout = ServiceMemoryLayout::ordinary_runtime();
const W: ServiceMemoryLayout = ServiceMemoryLayout::mask();
const L: ServiceMemoryLayout = ServiceMemoryLayout::tls();
const fn optional_bytes(range: Option<crate::service_memory::MemoryRange>) -> usize {
    match range {
        Some(range) => range.bytes() as usize,
        None => 0,
    }
}
#[repr(C)]
struct Slot<
    const I: usize,
    const O: usize,
    const S: usize,
    const M: usize,
    const P: usize,
    const A: usize,
    const T: usize,
> {
    pml4: Table,
    pdpt: Table,
    pd: Table,
    pt: [Table; T],
    image: Bytes<I>,
    exchange: Bytes<4096>,
    output: Bytes<O>,
    stack: Bytes<S>,
    metadata: Bytes<M>,
    pixels: Bytes<P>,
    scratch: Bytes<A>,
    trap: Bytes<16384>,
}
type Compositor = Slot<
    { C.image().bytes() as usize },
    { C.output().bytes() as usize },
    { C.stack().bytes() as usize },
    { C.metadata().bytes() as usize },
    { optional_bytes(C.pixels()) },
    { optional_bytes(C.scratch()) },
    { (C.end() as usize).div_ceil(2 * 1024 * 1024) },
>;
type Draft = Slot<
    { D.image().bytes() as usize },
    { D.output().bytes() as usize },
    { D.stack().bytes() as usize },
    { D.metadata().bytes() as usize },
    0,
    0,
    { (D.end() as usize).div_ceil(2 * 1024 * 1024) },
>;
type Home = Slot<
    { H.image().bytes() as usize },
    { H.output().bytes() as usize },
    { H.stack().bytes() as usize },
    { H.metadata().bytes() as usize },
    0,
    0,
    { (H.end() as usize).div_ceil(2 * 1024 * 1024) },
>;
type Runtime = Slot<
    { R.image().bytes() as usize },
    { R.output().bytes() as usize },
    { R.stack().bytes() as usize },
    { R.metadata().bytes() as usize },
    0,
    0,
    { (R.end() as usize).div_ceil(2 * 1024 * 1024) },
>;
type Application = Slot<
    { A.image().bytes() as usize },
    { A.output().bytes() as usize },
    { A.stack().bytes() as usize },
    { A.metadata().bytes() as usize },
    0,
    0,
    { (A.end() as usize).div_ceil(2 * 1024 * 1024) },
>;
type Tls = Slot<
    { L.image().bytes() as usize },
    { L.output().bytes() as usize },
    { L.stack().bytes() as usize },
    { L.metadata().bytes() as usize },
    0,
    0,
    { (L.end() as usize).div_ceil(2 * 1024 * 1024) },
>;
type Network = Slot<
    { N.image().bytes() as usize },
    { N.output().bytes() as usize },
    { N.stack().bytes() as usize },
    { N.metadata().bytes() as usize },
    0,
    0,
    { (N.end() as usize).div_ceil(2 * 1024 * 1024) },
>;
type OrdinaryRuntime = Slot<
    { O.image().bytes() as usize },
    { O.output().bytes() as usize },
    { O.stack().bytes() as usize },
    { O.metadata().bytes() as usize },
    0,
    0,
    { (O.end() as usize).div_ceil(2 * 1024 * 1024) },
>;
type Mask = Slot<
    { W.image().bytes() as usize },
    { W.output().bytes() as usize },
    { W.stack().bytes() as usize },
    { W.metadata().bytes() as usize },
    0,
    0,
    { (W.end() as usize).div_ceil(2 * 1024 * 1024) },
>;
impl<
    const I: usize,
    const O: usize,
    const S: usize,
    const M: usize,
    const P: usize,
    const A: usize,
    const T: usize,
> Slot<I, O, S, M, P, A, T>
{
    const fn empty() -> Self {
        Self {
            pml4: Table([0; 512]),
            pdpt: Table([0; 512]),
            pd: Table([0; 512]),
            pt: [Table([0; 512]); T],
            image: Bytes([0; I]),
            exchange: Bytes([0; 4096]),
            output: Bytes([0; O]),
            stack: Bytes([0; S]),
            metadata: Bytes([0; M]),
            pixels: Bytes([0; P]),
            scratch: Bytes([0; A]),
            trap: Bytes([0; 16384]),
        }
    }
    fn view(&mut self) -> View<'_> {
        View {
            pml4: &mut self.pml4,
            pdpt: &mut self.pdpt,
            pd: &mut self.pd,
            pt: &mut self.pt,
            image: &mut self.image.0,
            exchange: &mut self.exchange.0,
            output: &mut self.output.0,
            stack: &mut self.stack.0,
            metadata: &mut self.metadata.0,
            pixels: &mut self.pixels.0,
            scratch: &mut self.scratch.0,
            trap: &mut self.trap.0,
        }
    }
}
pub(super) struct View<'a> {
    pub pml4: &'a mut Table,
    pub pdpt: &'a mut Table,
    pub pd: &'a mut Table,
    pub pt: &'a mut [Table],
    pub image: &'a mut [u8],
    pub exchange: &'a mut [u8],
    pub output: &'a mut [u8],
    pub stack: &'a mut [u8],
    pub metadata: &'a mut [u8],
    pub pixels: &'a mut [u8],
    pub scratch: &'a mut [u8],
    pub trap: &'a mut [u8],
}
impl View<'_> {
    pub fn clear(&mut self) {
        self.pml4.0.fill(0);
        self.pdpt.0.fill(0);
        self.pd.0.fill(0);
        for table in &mut *self.pt {
            table.0.fill(0);
        }
        for bytes in [
            &mut *self.image,
            &mut *self.exchange,
            &mut *self.output,
            &mut *self.stack,
            &mut *self.metadata,
            &mut *self.pixels,
            &mut *self.scratch,
            &mut *self.trap,
        ] {
            bytes.fill(0);
        }
    }
}
struct Pool<T>(UnsafeCell<T>);
// A profile's independent claim is held until its Lease is dropped.
unsafe impl<T> Sync for Pool<T> {}
static COMPOSITOR: Pool<Compositor> = Pool(UnsafeCell::new(Compositor::empty()));
static DRAFT: Pool<Draft> = Pool(UnsafeCell::new(Draft::empty()));
static HOME: Pool<Home> = Pool(UnsafeCell::new(Home::empty()));
static RUNTIME: Pool<Runtime> = Pool(UnsafeCell::new(Runtime::empty()));
static APPLICATION: Pool<Application> = Pool(UnsafeCell::new(Application::empty()));
static NETWORK: Pool<Network> = Pool(UnsafeCell::new(Network::empty()));
static ORDINARY_RUNTIME: Pool<OrdinaryRuntime> = Pool(UnsafeCell::new(OrdinaryRuntime::empty()));
static MASK: Pool<Mask> = Pool(UnsafeCell::new(Mask::empty()));
static TLS: Pool<Tls> = Pool(UnsafeCell::new(Tls::empty()));
static COMPOSITOR_OWNED: AtomicBool = AtomicBool::new(false);
static DRAFT_OWNED: AtomicBool = AtomicBool::new(false);
static HOME_OWNED: AtomicBool = AtomicBool::new(false);
static RUNTIME_OWNED: AtomicBool = AtomicBool::new(false);
static APPLICATION_OWNED: AtomicBool = AtomicBool::new(false);
static NETWORK_OWNED: AtomicBool = AtomicBool::new(false);
static ORDINARY_RUNTIME_OWNED: AtomicBool = AtomicBool::new(false);
static MASK_OWNED: AtomicBool = AtomicBool::new(false);
static TLS_OWNED: AtomicBool = AtomicBool::new(false);
fn owned(profile: ServiceProfile) -> &'static AtomicBool {
    match profile {
        ServiceProfile::Compositor => &COMPOSITOR_OWNED,
        ServiceProfile::Draft => &DRAFT_OWNED,
        ServiceProfile::Home => &HOME_OWNED,
        ServiceProfile::Runtime => &RUNTIME_OWNED,
        ServiceProfile::Application => &APPLICATION_OWNED,
        ServiceProfile::Network => &NETWORK_OWNED,
        ServiceProfile::OrdinaryRuntime => &ORDINARY_RUNTIME_OWNED,
        ServiceProfile::Mask => &MASK_OWNED,
        ServiceProfile::Tls => &TLS_OWNED,
    }
}
pub(super) const fn reserved(profile: ServiceProfile) -> u32 {
    match profile {
        ServiceProfile::Compositor => core::mem::size_of::<Compositor>() as u32,
        ServiceProfile::Draft => core::mem::size_of::<Draft>() as u32,
        ServiceProfile::Home => core::mem::size_of::<Home>() as u32,
        ServiceProfile::Runtime => core::mem::size_of::<Runtime>() as u32,
        ServiceProfile::Application => core::mem::size_of::<Application>() as u32,
        ServiceProfile::Network => core::mem::size_of::<Network>() as u32,
        ServiceProfile::OrdinaryRuntime => core::mem::size_of::<OrdinaryRuntime>() as u32,
        ServiceProfile::Mask => core::mem::size_of::<Mask>() as u32,
        ServiceProfile::Tls => core::mem::size_of::<Tls>() as u32,
    }
}
pub(super) struct Lease {
    profile: ServiceProfile,
    cleared: bool,
}
impl Lease {
    pub fn claim(profile: ServiceProfile) -> Result<Self, DomainRefusal> {
        owned(profile)
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| DomainRefusal::InvalidMemory)?;
        Ok(Self {
            profile,
            cleared: false,
        })
    }
    pub fn profile(&self) -> ServiceProfile {
        self.profile
    }
    pub fn clear(&mut self) {
        self.view().clear();
        self.cleared = true;
    }
    pub fn view(&mut self) -> View<'_> {
        self.cleared = false;
        // SAFETY: the profile claim grants this lease unique mutable ownership;
        // the returned references cannot outlive the exclusive borrow of it.
        unsafe {
            match self.profile {
                ServiceProfile::Compositor => (&mut *COMPOSITOR.0.get()).view(),
                ServiceProfile::Draft => (&mut *DRAFT.0.get()).view(),
                ServiceProfile::Home => (&mut *HOME.0.get()).view(),
                ServiceProfile::Runtime => (&mut *RUNTIME.0.get()).view(),
                ServiceProfile::Application => (&mut *APPLICATION.0.get()).view(),
                ServiceProfile::Network => (&mut *NETWORK.0.get()).view(),
                ServiceProfile::OrdinaryRuntime => (&mut *ORDINARY_RUNTIME.0.get()).view(),
                ServiceProfile::Mask => (&mut *MASK.0.get()).view(),
                ServiceProfile::Tls => (&mut *TLS.0.get()).view(),
            }
        }
    }
    pub fn output(&self) -> &[u8] {
        // SAFETY: this live lease owns its store, no entry runs concurrently,
        // and this shared borrow excludes mutable access through the lease.
        unsafe {
            match self.profile {
                ServiceProfile::Compositor => &(*COMPOSITOR.0.get()).output.0,
                ServiceProfile::Draft => &(*DRAFT.0.get()).output.0,
                ServiceProfile::Home => &(*HOME.0.get()).output.0,
                ServiceProfile::Runtime => &(*RUNTIME.0.get()).output.0,
                ServiceProfile::Application => &(*APPLICATION.0.get()).output.0,
                ServiceProfile::Network => &(*NETWORK.0.get()).output.0,
                ServiceProfile::OrdinaryRuntime => &(*ORDINARY_RUNTIME.0.get()).output.0,
                ServiceProfile::Mask => &(*MASK.0.get()).output.0,
                ServiceProfile::Tls => &(*TLS.0.get()).output.0,
            }
        }
    }
    pub fn trap_top(&self) -> u64 {
        // SAFETY: only forming a raw address in this lease's live static store.
        let base = unsafe {
            match self.profile {
                ServiceProfile::Compositor => {
                    core::ptr::addr_of!((*COMPOSITOR.0.get()).trap.0) as u64
                }
                ServiceProfile::Draft => core::ptr::addr_of!((*DRAFT.0.get()).trap.0) as u64,
                ServiceProfile::Home => core::ptr::addr_of!((*HOME.0.get()).trap.0) as u64,
                ServiceProfile::Runtime => core::ptr::addr_of!((*RUNTIME.0.get()).trap.0) as u64,
                ServiceProfile::Application => {
                    core::ptr::addr_of!((*APPLICATION.0.get()).trap.0) as u64
                }
                ServiceProfile::Network => core::ptr::addr_of!((*NETWORK.0.get()).trap.0) as u64,
                ServiceProfile::OrdinaryRuntime => {
                    core::ptr::addr_of!((*ORDINARY_RUNTIME.0.get()).trap.0) as u64
                }
                ServiceProfile::Mask => core::ptr::addr_of!((*MASK.0.get()).trap.0) as u64,
                ServiceProfile::Tls => core::ptr::addr_of!((*TLS.0.get()).trap.0) as u64,
            }
        };
        base + 16384
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if !self.cleared {
            self.clear();
        }
        owned(self.profile).store(false, Ordering::Release);
    }
}
