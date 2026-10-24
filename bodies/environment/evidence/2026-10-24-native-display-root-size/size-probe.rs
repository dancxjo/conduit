use conduit_core as c;
use conduit_tm1637_wire::{installation::*, owner::WriteAttachment, WriteContract};
struct NoPins;
impl conduit_tm1637_wire::OpenDrain for NoPins {
 type Error=core::convert::Infallible;
 fn clock(&mut self,_:bool)->Result<(),Self::Error>{panic!("fixture must not perform IO")}
 fn data(&mut self,_:bool)->Result<(),Self::Error>{panic!("fixture must not perform IO")}
 fn read_data(&mut self)->Result<bool,Self::Error>{panic!("fixture must not perform IO")}
 fn half_period(&mut self)->Result<(),Self::Error>{panic!("fixture must not perform IO")}
}
fn main(){
 let source=std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
 let original:serde_json::Value=serde_json::from_str(&source).unwrap();
 let mut reply=if original.is_array(){original.as_array().unwrap().iter().find(|v|v.get("advertisement").is_some()).unwrap().clone()}else{original};
 let mut advertisement:c::HostAdvertisement=serde_json::from_value(reply["advertisement"].clone()).unwrap();
 let (host,boot)=(advertisement.host_id.as_str(),advertisement.boot_id.as_str());
        let identity = WriteNativeIdentity {
            host_id: host.into(),
            boot_id: boot.into(),
            base_id: "base/esp32/gpio18-gpio19".into(),
            provider_instance_id: "instance/esp32/tm1637".into(),
            provider_generation: 1,
            resource_pool_id: "pool/esp32/gpio18-gpio19".into(),
            resource_generation_id: c::ResourceGenerationId("generation/tm1637/1".into()),
            envelope_id: "envelope/esp32/tm1637".into(),
            artifact_id: "native/esp32-tm1637-root@1".into(),
            implementation_id: "esp32/tm1637-write@1".into(),
            base_implementation_id: "esp32/tm1637-base@1".into(),
        };
 let contract=WriteContract::prepare().unwrap();
 let ready=unsafe{ReadyWriteBase::new(identity,WriteAttachment{generation:1,resource_bytes:64},NoPins)}.unwrap();
 ready.append_to_advertisement(&mut advertisement,&contract).unwrap();
 advertisement.bases.sort_by(|a,b|a.base_id.cmp(&b.base_id));
 advertisement.resources.sort_by(|a,b|a.pool_id.cmp(&b.pool_id));
 let mut buffer=[0;8192];
 let bounded=serde_json_core::to_slice(&advertisement,&mut buffer).ok();
 let bytes=serde_json::to_vec(&advertisement).unwrap().len();
 let mut worst=advertisement.clone();worst.offer_generation=c::OfferGeneration(u64::MAX);
 let envelope=serde_json::json!({"protocol":1,"advertisement":worst,"friendly_label":"Conduit climate ESP32","verifying_key":vec![255u8;32],"freshness_sequence":u64::MAX});
 println!("{}",serde_json::json!({"proof":"maximum-counter-envelope-size-fixture","admission_bytes":serde_json::to_vec(&envelope).unwrap().len(),"bounded_encoder_bytes":serde_json_core::to_slice(&envelope,&mut [0u8; 8512]).ok()}));

 reply["advertisement"]=serde_json::to_value(&advertisement).unwrap();
 println!("{}",serde_json::json!({"proof":"recorded-boot-size-fixture","advertisement_bytes":bytes,"bounded_encoder_bytes":bounded,"complete_root_frame_bytes":serde_json::to_vec(&reply).unwrap().len(),"physical_io":false}));
}
