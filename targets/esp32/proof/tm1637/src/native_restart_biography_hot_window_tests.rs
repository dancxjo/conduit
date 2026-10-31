//! Actual shared arbitration types, with CPU time/media stand-ins. This does
//! not qualify native SDK timing, interrupt state or physical sensor/display.
use super::private::ProfileAccess;
use super::*;
use crate::{native_flash_jobs as jobs, native_flash_window as window};
struct Work<'a> {
    transaction: Transaction,
    store: &'a mut Store,
    access: ProfileAccess,
}
impl jobs::Job for Work<'_> {
    fn step(&self) -> u32 {
        self.transaction.writer.as_ref().unwrap().step()
    }
}

#[test]
fn actual_mailbox_denies_every_flash_step_during_owned_pulse() {
    let (mut store, token, host, root, evidence, _) = fixture();
    let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
    let receipt = endorsed(&transaction);
    transaction.begin(&mut store, receipt).unwrap();
    let mut work = Work {
        transaction,
        store: &mut store,
        access: ProfileAccess {
            calls: 0,
            fault: None,
            media: None,
        },
    };
    let mut mailbox = jobs::Mailbox::<Work<'_>, bool, &'static str>::new();
    let (mut submit, mut execute) = mailbox.split().unwrap();
    let gate = window::Gate::new(1000, 20, 1000).unwrap();
    let mut now = 0;
    let mut denied = 0;
    loop {
        let wait = gate.begin_wait(now, now + 1000).unwrap();
        if work.transaction.writer.as_ref().unwrap().needs_chunk() {
            let quiet = wait.try_flash(now, 1000).unwrap();
            work.transaction.prepare_chunk(work.store).unwrap();
            drop(quiet);
        }
        let before = work.access.calls;
        let id = submit.submit(work, 1000).ok().unwrap();
        let pulse = gate.begin_pulse(now, now + 20).unwrap();
        let result = execute.execute(&wait, now, |_, _| panic!("pulse must exclude all effects"));
        assert!(matches!(
            result,
            Err(jobs::Refusal::Window(window::Refusal::Overlap))
        ));
        denied += 1;
        drop(pulse);
        execute
            .execute(&wait, now, |work, _permit| {
                assert!(matches!(
                    gate.begin_pulse(now, now + 20),
                    Err(window::Refusal::Overlap)
                ));
                work.transaction.advance(work.store, &mut work.access)
            })
            .unwrap();
        let complete = submit.take(id).unwrap();
        assert!(matches!(complete.progress, jobs::Progress::AdvancedOne));
        let done = complete.result.unwrap();
        work = complete.owner;
        assert_eq!(work.access.calls, before + 1);
        drop(wait);
        now += 1001;
        if done {
            break;
        }
    }
    let wait = gate.begin_wait(now, now + 1000).unwrap();
    let quiet = wait.try_flash(now, 1000).unwrap();
    let mut token = work.transaction.finish(work.store).unwrap();
    drop(quiet);
    drop(wait);
    let owner = restore_store(work.store, &mut token, &host.host_id, &host.boot_id, &root).unwrap();
    assert_eq!(owner.evidence(), &evidence);
    assert_eq!(denied, work.access.calls);
    println!("hot_custody_cpu_arbitration: physical_steps={} pulse_owned_refusals={} one_permit_one_step=true native_timing_not_proven=true",work.access.calls,denied);
}

#[test]
fn partial_sector_and_program_cuts_keep_owner_and_select_only_complete_snapshots() {
    fn run(
        transaction: &mut Transaction,
        store: &mut Store,
        access: &mut ProfileAccess,
    ) -> Result<(), &'static str> {
        loop {
            if transaction.writer.as_ref().unwrap().needs_chunk() {
                transaction.prepare_chunk(store)?;
            }
            if transaction.advance(store, access)? {
                return Ok(());
            }
        }
    }
    let (mut store, token, _, _, _, _) = fixture();
    let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
    let receipt = endorsed(&transaction);
    transaction.begin(&mut store, receipt).unwrap();
    let mut all = ProfileAccess {
        calls: 0,
        fault: None,
        media: None,
    };
    run(&mut transaction, &mut store, &mut all).unwrap();
    for cut in 1..=all.calls {
        for mode in 0..3 {
            let (mut store, token, host, root, evidence, admissions) = fixture();
            let generation = token.generation;
            let summary = store.custody.as_ref().unwrap().summary.clone();
            let mut transaction = Transaction::prepare(&mut store, token).ok().unwrap();
            let receipt = endorsed(&transaction);
            transaction.begin(&mut store, receipt).unwrap();
            let mut access = ProfileAccess {
                calls: 0,
                fault: Some((cut, mode)),
                media: None,
            };
            assert!(run(&mut transaction, &mut store, &mut access).is_err());
            let mut token = transaction.cancel(&mut store).unwrap();
            assert_eq!(token.generation, generation);
            // Explicitly aliased CPU media simulates losing RAM and reopening the
            // two banks. It is never a second production physical storage owner.
            let region = BodyRegion {
                media: access.media.unwrap(),
            };
            let mut reopened = Checkpoint::open(region).unwrap();
            let next = reopened.generation().unwrap();
            assert!(next == generation || next == generation + 1);
            let mut seed = PrivateSeed([0; 32]);
            let (resident, _) = decode_checkpoint(
                &mut reopened,
                &store.binding,
                &evidence.body_id,
                &mut seed.0,
                None,
            )
            .unwrap();
            assert!(resident.retained.evidence == evidence && resident.summary == summary);
            assert_eq!(
                serde_json::to_vec(&resident.retained.admissions).unwrap(),
                admissions
            );
            assert!(resident.local_count == 0 || resident.local_count == summary.count);
            let owner =
                restore_store(&mut store, &mut token, &host.host_id, &host.boot_id, &root).unwrap();
            assert_eq!(owner.evidence(), &evidence);
        }
    }
    println!("hot_custody_nor_cuts: operations={} modes=3 cases={} old_or_complete_endorsed_successor_only=true",all.calls,all.calls*3);
}
