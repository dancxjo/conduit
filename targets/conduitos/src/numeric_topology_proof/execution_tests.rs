use super::*;
use alloc::vec::Vec;
use resources::{File, PreparedIngress};
static STORAGE: storage::StaticStorage = storage::StaticStorage::new();
#[test]
#[ignore = "explicit synthetic local fixture; ordinary owner execution, not guest boot"]
fn synthetic_topology_executes_to_full_drain_in_caller_placed_storage() {
    std::thread::Builder::new().stack_size(PREPARATION_STACK_BYTES).spawn(||{
        let directory=std::path::PathBuf::from(std::env::var("CONDUIT_NUMERIC_GUEST_FIXTURE").unwrap());
        let source=std::fs::read(directory.join("checked-epoch-source.conduit")).unwrap();
        let image=std::fs::read(directory.join("sealed-epoch-plan.json")).unwrap();
        let definition=std::fs::read_to_string(directory.join("native-definition.conduit")).unwrap();
        let recipe=std::fs::read(directory.join("preparation-recipe.json")).unwrap();
        let mut contents=Vec::new();
        for entry in std::fs::read_dir(&directory).unwrap() {
            let entry=entry.unwrap();
            let name=entry.file_name().into_string().unwrap();
            if name.ends_with(".bin"){contents.push((name,std::fs::read(entry.path()).unwrap()));}
        }
        let files:Vec<_>=contents.iter().map(|(name,bytes)|File{name,bytes}).collect();
        let topology=PreparedTopology::prepare(Materials{source:&source,reference_image:&image,native_definition:&definition,recipe:&recipe},"synthetic-execution/host".into(),"synthetic-execution/boot".into()).unwrap();
        let ingress=PreparedIngress::prepare(&topology,&files).unwrap();
        let mut prepared=execution::PreparedExecution::prepare(&topology,ingress,STORAGE.claim().unwrap()).unwrap();
        let receipt=prepared.run().unwrap();
        assert!(receipt.expression_calls>=218);
        assert!(receipt.output_bytes>0 && receipt.output_bytes<=storage::CELL_BYTES);
        assert!(receipt.peak_cells<=storage::SLOTS as u16);
        std::eprintln!("synthetic ordinary fresh-Boot full drain: {receipt:?}; no guest/real-time/model claim");
    }).unwrap().join().unwrap();
}
