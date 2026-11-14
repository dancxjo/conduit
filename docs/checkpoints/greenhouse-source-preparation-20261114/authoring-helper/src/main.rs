use conduit_mcu_source_build as source_build;
#[path="/home/dancxjo/Documents/Codex/2026-10-26/goal-re-conduit-mcu-thermometer-fit/work/environment-extraction-integration-20261111/targets/mcu/source-build/tests/fixtures/environmental_exchange_source.rs"] mod fixture;
fn main(){
 let out=std::path::PathBuf::from(std::env::args_os().nth(1).expect("new candidate directory"));
 let commit=std::env::args().nth(2).expect("checked source commit");
 let f=fixture::selected_source();let compiled=f.compile().expect("whole checked Source");
 let identity=format!("compile-probe:{commit}:{}",f.source_sha256);
 let packages=conduit_host_make::MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
 let catalog=conduit_host_make::MakeCatalog::canonical().with_packages(&packages);
 let text=std::fs::read_to_string(out.join("green-neutral-candidate.host.conduit")).unwrap();
 let checked=conduit_host_make::check_host_configuration(conduit_host_make::parse_host_configuration_conduit(&text).unwrap(),&catalog,&packages).unwrap();
 let (image,description)=conduit_host_make::build_default_host_image(checked.profile().clone(),&catalog,&packages,&conduit_host_make::BuildInputs{source_identity:identity.clone(),toolchain_available:true}).unwrap();
 conduit_host_make::verify_configuration_image(&checked,&image,&catalog).unwrap();
 for (name,data) in [("workset.conduit",f.source.as_bytes()),("specializations.json",f.typed_receipt.as_slice()),("compile-image.json",description.as_slice()),("compile-probe-source-identity.txt",identity.as_bytes())]{
  assert!(!out.join(name).exists(),"never overwrite prior evidence");std::fs::write(out.join(name),data).unwrap();
 }
 println!("checked source_sha256={} gears={} configuration={} image={} UNINSTALLED_no_admission_or_deployment",f.source_sha256,compiled.entries[0].plot.expanded.gears.len(),checked.configuration_id(),image.manifest.image_id);
}
