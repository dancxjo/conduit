use conduit_core::validate_canonical_structured_value;
fn main() {
 let args: Vec<_> = std::env::args().collect();
 assert!(matches!(args.len(), 3 | 4));
 let phase = args.get(3).map_or(3u64, |s| s.parse().unwrap());
 let log = std::fs::read_to_string(&args[1]).unwrap();
 let expected = std::fs::read(&args[2]).unwrap();
 assert_eq!(expected.len(), 1536);
 assert!(!log.contains("CONDUIT_PROTOCOL_REFUSAL "));
 assert!(!log.contains("panicked at"));
 let lines: Vec<_> = log.lines().collect();
 let outputs: Vec<_> = lines.iter().enumerate().filter(|(_, l)| l.starts_with("CONDUIT_PROTOCOL_OUTPUT port=ended ")).collect();
 assert_eq!(outputs.len(), 1);
 assert_eq!(lines.iter().filter(|line| line.starts_with("CONDUIT_PROTOCOL_OUTPUT ")).count(), 4);
 assert!(lines.iter().any(|line| line.starts_with("CONDUIT_PROTOCOL_PLAN ")));
 assert!(lines.iter().any(|line| line.starts_with("CONDUIT_PROTOCOL_PLAY ")));
 let position = outputs[0].0;
 let hex = lines[position + 1].trim();
 assert_eq!(hex.len() % 2, 0);
 assert!(log.ends_with("\n"));
 let bytes: Vec<_> = hex.as_bytes().as_chunks::<2>().0.iter().map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect();
 let state = validate_canonical_structured_value(&bytes).unwrap();
 assert_eq!(state.record_field("phase").unwrap().unwrap().primitive_bytes("value/u64").unwrap(), phase.to_le_bytes());
 assert_eq!(state.record_field("terminal").unwrap().unwrap().primitive_bytes("value/bool").unwrap(), [1]);
 let first = u64::from_le_bytes(state.record_field("first-frame").unwrap().unwrap().primitive_bytes("value/u64").unwrap().try_into().unwrap());
 let observed: Vec<_> = lines.iter().enumerate().filter(|(_,l)| l.starts_with("CONDUIT_PROTOCOL_OUTPUT port=observed ")).collect();
 assert_eq!(observed.len(),3);
 for (ordinal, (index,_)) in observed.iter().enumerate() {
  let encoded: Vec<_> = lines[index+1].as_bytes().as_chunks::<2>().0.iter().map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(),16).unwrap()).collect();
  let result = validate_canonical_structured_value(&encoded).unwrap();
  let tag = ["snapshot","completed","drained"][ordinal];
  assert_eq!(result.variant_tag().unwrap(),tag);
  if tag == "completed" {
   let packets = result.variant_payload(tag).unwrap().unwrap().record_field("packets").unwrap().unwrap();
   assert_eq!(packets.collection_length().unwrap(),72);
   for i in 0..72u16 {
    let packet = packets.collection_index(i).unwrap().unwrap();
    for (field,expected) in [("frame",(first+u64::from(i))%2048),("requested",192),("actual",192)] {
     assert_eq!(packet.record_field(field).unwrap().unwrap().primitive_bytes("value/u64").unwrap(),expected.to_le_bytes());
    }
   }
  }
 }
 assert!(observed[2].0 < position);
 let pcm = state.record_field("pcm").unwrap().unwrap();
 let packets = pcm.record_field("packets").unwrap().unwrap();
 assert_eq!(packets.collection_length().unwrap(), 8);
 let mut retained = Vec::new();
 for i in 0..8 { retained.extend_from_slice(packets.collection_index(i).unwrap().unwrap().record_field("wire").unwrap().unwrap().primitive_bytes("value/bytes").unwrap()); }
 assert_eq!(retained, expected);
 let retired: Vec<_> = lines.iter().enumerate().filter(|(_,l)| l.starts_with("CONDUIT_PROTOCOL_RETIRED complete")).collect();
 assert_eq!(retired.len(), 1);
 assert!(retired[0].0 > position);
 println!("checked terminal phase={phase} original PCM retained, retirement observed; wire verification required separately");
}
