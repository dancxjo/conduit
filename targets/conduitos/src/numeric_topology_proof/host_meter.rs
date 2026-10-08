//! Isolated development meter for the same synthetic portable preparation/run.
//! A hosted allocator receipt is not an emulator or physical target receipt.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]
#[cfg(not(target_os = "none"))]
mod hosted {
    use conduitos::numeric_topology_proof::{
        self as proof,
        execution::PreparedExecution,
        resources::{File, PreparedIngress},
    };

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Meter;
    static LIVE: AtomicUsize = AtomicUsize::new(0);
    static PEAK: AtomicUsize = AtomicUsize::new(0);
    static REQUESTS: AtomicUsize = AtomicUsize::new(0);
    fn accepted(bytes: usize) {
        let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
        PEAK.fetch_max(live, Ordering::Relaxed);
    }
    unsafe impl GlobalAlloc for Meter {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            REQUESTS.fetch_add(1, Ordering::Relaxed);
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                accepted(layout.size());
            }
            pointer
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            REQUESTS.fetch_add(1, Ordering::Relaxed);
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                accepted(layout.size());
            }
            pointer
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, length: usize) -> *mut u8 {
            REQUESTS.fetch_add(1, Ordering::Relaxed);
            let resized = unsafe { System.realloc(pointer, layout, length) };
            if !resized.is_null() {
                LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
                accepted(length);
            }
            resized
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            unsafe { System.dealloc(pointer, layout) }
        }
    }
    #[global_allocator]
    static METER: Meter = Meter;
    static STORAGE: proof::storage::StaticStorage = proof::storage::StaticStorage::new();
    pub fn run_meter() {
        // Reserve the exact selected finite preparation stack before entering the
        // measurement. It is external to heap peak and reported separately.
        std::thread::Builder::new()
            .stack_size(proof::PREPARATION_STACK_BYTES)
            .spawn(run)
            .unwrap()
            .join()
            .unwrap();
    }
    fn run() {
        let directory = std::path::PathBuf::from(
            std::env::var("CONDUIT_NUMERIC_GUEST_FIXTURE")
                .expect("explicit synthetic fixture directory"),
        );
        let source = std::fs::read(directory.join("checked-epoch-source.conduit")).unwrap();
        let image = std::fs::read(directory.join("sealed-epoch-plan.json")).unwrap();
        let definition =
            std::fs::read_to_string(directory.join("native-definition.conduit")).unwrap();
        let recipe = std::fs::read(directory.join("preparation-recipe.json")).unwrap();
        let mut contents = Vec::new();
        for entry in std::fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            if name.ends_with(".bin") {
                contents.push((name, std::fs::read(entry.path()).unwrap()));
            }
        }
        let files: Vec<_> = contents
            .iter()
            .map(|(name, bytes)| File { name, bytes })
            .collect();
        let material_live = LIVE.load(Ordering::Relaxed);
        let start = std::time::Instant::now();
        let topology = proof::PreparedTopology::prepare(
            proof::Materials {
                source: &source,
                reference_image: &image,
                native_definition: &definition,
                recipe: &recipe,
            },
            "synthetic-host-meter".into(),
            "synthetic-meter-boot".into(),
        )
        .unwrap();
        let planned = start.elapsed();
        let ingress = PreparedIngress::prepare(&topology, &files).unwrap();
        let resource_bytes = ingress.raw_resource_bytes;
        let descriptor_bytes = ingress.descriptor_inline_bytes;
        let mut execution =
            PreparedExecution::prepare(&topology, ingress, STORAGE.claim().unwrap()).unwrap();
        let preparation = start.elapsed();
        let peak = PEAK.load(Ordering::Relaxed);
        let retained = LIVE.load(Ordering::Relaxed);
        let before = REQUESTS.load(Ordering::Relaxed);
        let started = std::time::Instant::now();
        let result = execution.run();
        let elapsed = started.elapsed();
        let after = REQUESTS.load(Ordering::Relaxed);
        println!(
            "synthetic_host_meter result={result:?} planning={planned:?} preparation={preparation:?} execution={elapsed:?} material_live={material_live} peak_heap={peak} retained_heap={retained} allocation_requests_during_run={} static_store={} preparation_stack={} resource_bytes={resource_bytes} inline_descriptors={descriptor_bytes}",
            after - before,
            std::mem::size_of::<proof::storage::StaticStorage>(),
            proof::PREPARATION_STACK_BYTES
        );
        assert_eq!(after, before, "portable run attempted allocation");
        assert!(result.is_ok(), "portable run must fully drain");
    }
}
#[cfg(not(target_os = "none"))]
fn main() {
    hosted::run_meter();
}
#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
extern "C" fn conduitos_start() -> ! {
    conduitos::arch::deterministic_exit(false)
}
#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    conduitos::arch::deterministic_exit(false)
}
