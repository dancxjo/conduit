//! Production-emitted wrappers around a terminal-bearing test Back. Supported
//! expression engines still reject planned terminal laws; this isolates exact
//! forwarding without pretending that a new expression semantic was added.
use crate::resident_expression_tests::compiled_rust;
#[test]
fn emitted_resident_wrappers_preserve_actual_nonempty_laws_without_withdrawal() {
    let mut code = String::from(
        r#"
mod entry_0 {
use conduit_kernel::scheduler::*;
struct EngineBack;
type Back=EngineBack;
impl<const P:usize> StepBack<P> for EngineBack {
 fn terminal_transductions(&self)->[Option<AssignedTerminalTransduction>;P] {
  core::array::from_fn(|p|(p==0).then_some(AssignedTerminalTransduction {
   input:conduit_kernel::PortId(0),output:conduit_kernel::PortId(0),
   normal_close:AssignedNormalCloseTransduction::Consume,
   abnormal:AssignedAbnormalTransduction::Recover,
   cancellation:AssignedCancellationTransduction::NotCancellable,
  }))
 }
 fn step(&mut self,_:&mut StepIo<P>,_:&StepInputBytes<'_,P>)->StepOutcome {StepOutcome::Await}
}
pub struct Initialized {back:Back,pub available:bool}
impl Initialized {
 pub fn prepare()->Self{Self{back:EngineBack,available:true}}
 pub(super) fn borrow_prechecked(&mut self)->BorrowedBack<'_>{self.available=false;BorrowedBack{owner:self}}
"#,
    );
    crate::resident_offer::append_terminal_inspection(&mut code);
    code.push_str(r#"
}
pub struct BorrowedBack<'a>{owner:&'a mut Initialized}
impl BorrowedBack<'_>{pub fn finish_retired(self)->Result<(),Self>{self.owner.available=true;Ok(())}}
"#);
    crate::resident_back::forward(&mut code, "BorrowedBack<'_>", "self.owner.back", false);
    code.push_str("}\npub struct Initialized {entry_0:Option<entry_0::Initialized>}\n");
    crate::resident_library_borrow::append_back(&mut code, 1, false);
    let probe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../architecture/plot/tests/common/allocation_probe.rs");
    let exercise = format!(
        r#"
#[path={probe:?}]mod probe;
#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;
fn main(){{
 let mut owner=Initialized{{entry_0:Some(entry_0::Initialized::prepare())}};
 let expected=owner.entry_0.as_ref().unwrap().terminal_transductions::<2>();
 assert!(expected[0].is_some());assert!(expected[1].is_none());
 let((),cost)=probe::observe(||{{
  let loans=PreparedResidentLoans{{owner:&mut owner,nodes:[Some(3)]}};
  assert_eq!(loans.terminal_transductions::<2>(),[Some(expected)]);
  let entries=loans.split();
  assert_eq!(entries[0].as_ref().unwrap().node(),3);
  assert_eq!(entries[0].as_ref().unwrap().terminal_transductions::<2>(),expected);
  drop(entries);
 }});
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
 assert!(owner.entry_0.as_ref().unwrap().available);
 let((),cost)=probe::observe(||{{
  let loans=PreparedResidentLoans{{owner:&mut owner,nodes:[Some(3)]}};
  let mut entries=loans.split();
  let(node,back)=entries[0].take().unwrap().bind();
  assert_eq!(node,3);
  assert_eq!(conduit_kernel::scheduler::StepBack::<2>::terminal_transductions(&back),expected);
  assert!(back.finish_retired().is_ok());
 }});
 assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);
 assert!(owner.entry_0.as_ref().unwrap().available);
}}
"#,
        probe = probe.to_str().unwrap()
    );
    compiled_rust::verify_with_libraries(
        &code,
        &exercise,
        "resident-terminal-wrappers",
        &["conduit_kernel", "conduit_plan_lowering"],
    );
}
