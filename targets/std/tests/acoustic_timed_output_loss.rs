//! Actual authored PCM reaches an admitted Host Call sink; provider loss stays failure.
use conduit_core::*;
use conduit_kernel::{Failure, FailureCode, HostCallDisposition, HostCallOutcome};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common_acoustic_timed/boundary.rs"]
mod boundary;
#[path = "common_acoustic_timed/output_sink.rs"]
mod output_sink;
#[path = "common_acoustic_timed/pcm_loss_plan.rs"]
mod pcm_loss_plan;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn pcm_sink_provider_loss_after_one_commit_is_terminal_without_growth_or_retry() {
    std::thread::Builder::new().stack_size(8*1024*1024).spawn(|| {
        let prepared=pcm_loss_plan::prepare();
        let mut kernel=boundary::prepare(prepared.plan,&prepared.front,&prepared.registry);
        let input=&prepared.front.inputs()[0].port_id;let out=&prepared.front.outputs()[0].port_id;
        let ticks=prepared.items.iter().map(|encoded|ValuePayload {value_kind:prepared.front.inputs()[0].value_kind.clone(),encoded:encoded.clone()}).collect::<Vec<_>>();
        let mut output=ValuePayload {value_kind:prepared.front.outputs()[0].value_kind.clone(),encoded:Vec::with_capacity(2077)};
        let mut reply=Vec::with_capacity(2077);let mut requests=0;let mut commits=0;let mut lost=false;let mut loss_token=None;
        let (_,storage)=allocation_probe::observe(|| {
            kernel.start().unwrap();
            for frame in 0..2 {
                assert!(matches!(kernel.admit_input(input,frame,&ticks[frame as usize]).unwrap(),conduit_kernel::scheduler::RemoteIngressOutcome::Accepted {sequence} if sequence==frame));
                let mut finished=false;
                for _ in 0..5000 {
                    if let Err(error)=kernel.step() {
                        assert!(matches!(error,conduit_composite::KernelCompositeError::Execution {reason:conduit_composite::ChildExecutionError::Scheduler(conduit_kernel::scheduler::SchedulerError::BackFailed(Failure {code:FailureCode::HostCallFailed,detail:75})),..}));
                        assert_eq!(frame,1);lost=true;finished=true;break;
                    }
                    if let Some(request)=kernel.next_host_request() {
                        let obligation=kernel.host_request_obligation(&request).unwrap();
                        let admitted=kernel.admit_host_request(&request,&obligation.host,&obligation.resources,&obligation.authorities).unwrap();
                        let bytes=kernel.host_request_input(&admitted).unwrap();
                        let (header,samples)=conduit_audio::PcmFrameHeader::decode_frame(bytes).unwrap();
                        assert_eq!(header.start_frame(),frame);assert_eq!(header.frame_count(),1);assert_eq!(header.sample_rate_hz(),8000);
                        assert_eq!(samples,if frame==0 {24_i16.to_le_bytes()} else {104_i16.to_le_bytes()});
                        requests+=1;
                        if frame==0 {reply.clear();reply.extend_from_slice(bytes);kernel.complete_host_call_bytes(&admitted,&reply).unwrap();}
                        else {kernel.complete_host_call(&admitted,HostCallOutcome {disposition:HostCallDisposition::Failed,output:None,failure:Some(Failure {code:FailureCode::HostCallFailed,detail:75})}).unwrap();loss_token=Some(admitted);}
                    }
                    if let Some(sequence)=kernel.output_into(out,&mut output).unwrap() {
                        assert_eq!(frame,0);assert_eq!(sequence,0);commits+=1;kernel.complete_output(out,sequence).unwrap();finished=true;break;
                    }
                }
                assert!(finished);
            }
            assert!(lost);assert_eq!((requests,commits),(2,1));
            assert!(kernel.next_host_request().is_none());
            assert!(matches!(kernel.complete_host_call(loss_token.as_ref().unwrap(),HostCallOutcome {disposition:HostCallDisposition::Completed,output:None,failure:None}),Err(conduit_composite::KernelCompositeError::InvalidHostCallToken)));
        });
        assert_eq!((storage.allocations,storage.reallocations),(0,0),"{storage:?}");
        let signs=kernel.signs();
        assert_eq!(signs.values().flatten().filter(|sign|sign.kind==conduit_kernel::KernelEventKind::BackFailed).count(),1);
        assert!(!signs.values().flatten().any(|sign|sign.kind==conduit_kernel::KernelEventKind::RunCancelled));
        println!("pcm-loss requests={requests} commits={commits} plays={:?} storage={storage:?}",kernel.active_plays());
    }).unwrap().join().unwrap();
}
