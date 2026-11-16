//! Operational adapter for an already qualified application; no firmware compilation or flashing.
use conduit_host_make as h;
use sha2::{Digest,Sha256};
#[path="/home/dancxjo/Documents/Codex/2026-10-26/goal-re-conduit-mcu-thermometer-fit/work/environment-extraction-integration-20261111/tools/xtask/src/commands/host_esp32_native_seed.rs"] mod canonical_seed;
fn main()->Result<(),Box<dyn std::error::Error>> {
 let a:Vec<_>=std::env::args().collect();
 if a.len()!=6{return Err("profile, sealed artifact directory, private input, private sector output required".into())}
 #[derive(serde::Deserialize)] struct Reviewed { image_id:String, configuration_id:String, application_bytes:usize, application_sha256:String }
 let path=std::path::Path::new(&a[5]);if std::fs::metadata(path)?.len()>65536{return Err("review exceeds finite extent".into())}
 let review:Reviewed=serde_json::from_slice(&std::fs::read(path)?)?;
 if review.application_bytes==0 || review.application_bytes>4*1024*1024{return Err("application exceeds physical profile".into())}
 let profile=std::fs::read_to_string(&a[1])?;
 let packages=h::MakePackageSet::compose(&[&conduit_host_esp32_make::Esp32MakePackage]).map_err(|e|format!("{e:?}"))?;
 let catalog=h::MakeCatalog::canonical().with_packages(&packages);
 let checked=h::check_host_configuration(h::parse_host_configuration_conduit(&profile).map_err(|e|format!("{e:?}"))? ,&catalog,&packages).map_err(|e|format!("{e:?}"))?;
 let dir=std::path::Path::new(&a[2]);
 let image:h::HostImage=serde_json::from_slice(&std::fs::read(dir.join("resolved-image.json"))?)?;
 h::verify_configuration_image(&checked,&image,&catalog)?;
 if image.manifest.image_id!=review.image_id || checked.configuration_id()!=review.configuration_id {return Err("independently reviewed Blue IMAGE/configuration mismatch".into())}
 let app=std::fs::read(dir.join("host.bin"))?;
 if app.len()!=review.application_bytes || format!("{:x}",Sha256::digest(&app))!=review.application_sha256 {return Err("qualified application bytes mismatch".into())}
 canonical_seed::compose(&checked,&image.manifest.image_id,std::path::Path::new(&a[3]),std::path::Path::new(&a[4]),dir)?;
 println!("Private bootstrap prepared against reviewed application; no flash write or admission."); Ok(())
}
