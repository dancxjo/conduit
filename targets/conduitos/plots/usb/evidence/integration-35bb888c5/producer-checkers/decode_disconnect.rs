use conduit_core::validate_canonical_structured_value;
fn main() {
 let log=std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
 assert!(log.contains("CONDUIT_PROTOCOL_RETIRED complete"));
 assert!(!log.contains("CONDUIT_PROTOCOL_REFUSAL "));
 let lines:Vec<_>=log.lines().collect();let mut count=0;
 for (index,line) in lines.iter().enumerate(){
  if !line.starts_with("CONDUIT_PROTOCOL_OUTPUT port=observation "){continue;}
  let hex=lines[index+1];assert_eq!(hex.len()%2,0);
  let bytes:Vec<u8>=hex.as_bytes().chunks_exact(2).map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect();
  let value=validate_canonical_structured_value(&bytes).unwrap();
  if count==0 {
   assert_eq!(value.variant_tag().unwrap(),"transmitted");let p=value.variant_payload("transmitted").unwrap().unwrap();
   for (name,n) in [("ordinal",0u64),("requested",12),("actual",12)]{assert_eq!(p.record_field(name).unwrap().unwrap().primitive_bytes("value/u64").unwrap(),n.to_le_bytes());}
  } else {assert_eq!(count,1);assert_eq!(value.variant_tag().unwrap(),"provider-lost");assert!(value.variant_payload("provider-lost").unwrap().unwrap().primitive_bytes("value/empty").unwrap().is_empty());}
  count+=1;
 }
 assert_eq!(count,2);println!("{{\"serial_results\":2,\"transmitted\":1,\"terminal\":\"provider-lost\",\"retired\":true}}");
}
