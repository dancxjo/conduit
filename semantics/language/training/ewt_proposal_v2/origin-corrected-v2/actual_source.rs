use conduit_core::*;
use conduit_plot::*;
use std::io::{BufRead,BufReader};
fn field<'a>(v:ValidatedCanonicalStructuredValue<'a>,path:&[&str])->ValidatedCanonicalStructuredValue<'a>{path.iter().fold(v,|v,n|v.record_field(n).unwrap().unwrap())}
fn slot(root:&[u8],v:ValidatedCanonicalStructuredValue<'_>)->usize { let b=v.value_node();assert_eq!(b.len(),13); b.as_ptr() as usize-root.as_ptr() as usize+5 }
fn fields(root:&[u8],v:ValidatedCanonicalStructuredValue<'_>,path:&[&str])->usize{slot(root,field(v,path))}
fn collection(root:&[u8],v:ValidatedCanonicalStructuredValue<'_>,path:&[&str])->Vec<usize>{field(v,path).collection_elements().unwrap().map(|x|slot(root,x.unwrap())).collect()}
fn write(b:&mut[u8],p:usize,n:u64){b[p..p+8].copy_from_slice(&n.to_le_bytes());}
fn main(){
 let old=std::path::Path::new("/home/dancxjo/conduit-4907-evaluation-identity/work/lexical-proposer-next");
 let out=std::path::Path::new("/home/dancxjo/conduit-4907-evaluation-identity/target/debug/build/conduit-language-70b796a2da20ac23/out");
 let program=|n:&str|PortableExpressionProgram::from_canonical_hex(&std::fs::read_to_string(out.join(format!("{n}.hex"))).unwrap()).unwrap();
 let context=program("proposal_window8_feature_context");let values=program("proposal_window8_v2_feature_values");
 let mut bytes=context.evaluate(&std::fs::read(old.join("new-feature-vocative.bin")).unwrap()).unwrap();
 let v=validate_canonical_structured_value(&bytes).unwrap();
 let top=fields(&bytes,v,&["top_pos"]);let next=fields(&bytes,v,&["next_pos"]);
 let depth=fields(&bytes,v,&["query","raw","state","depth"]);let unread=fields(&bytes,v,&["query","raw","state","unread"]);let count=fields(&bytes,v,&["query","raw","projection","token_count"]);
 let stack=collection(&bytes,v,&["query","raw","state","stack"]);let heads=collection(&bytes,v,&["query","raw","state","heads"]);
 let choices=collection(&bytes,v,&["query","raw","choices"]);
 let origins=collection(&bytes,v,&["query","origins","raw","origins"]);
 let future_count=fields(&bytes,v,&["future","count"]);let future=collection(&bytes,v,&["future","codes"]);
 let mut token_codes=Vec::new();for token in field(v,&["query","raw","projection","tokens"]).collection_elements().unwrap(){token_codes.push(collection(&bytes,token.unwrap(),&["codes"])[0]);}
 let mut evaluator=PreparedPortableExpressionEvaluator::new(&values).unwrap();
 let mut total=0;let started=std::time::Instant::now();
 for line in BufReader::new(std::fs::File::open("work/oracle-feature-parity/vectors.jsonl").unwrap()).lines(){
  let row:serde_json::Value=serde_json::from_str(&line.unwrap()).unwrap();let s=&row["state"];let n=s["n"].as_u64().unwrap();let u=s["unread"].as_u64().unwrap();let st=s["stack"].as_array().unwrap();let t=st.last().unwrap().as_u64().unwrap();let pos=row["pos"].as_array().unwrap();
  write(&mut bytes,top,if t==8{17}else{pos[t as usize].as_u64().unwrap()});write(&mut bytes,next,if u==n{17}else{pos[u as usize].as_u64().unwrap()});write(&mut bytes,depth,st.len() as u64);write(&mut bytes,unread,u);write(&mut bytes,count,n);
  for(i,p)in stack.iter().enumerate(){write(&mut bytes,*p,st.get(i).and_then(|x|x.as_u64()).unwrap_or(8));}
  for(i,p)in heads.iter().enumerate(){write(&mut bytes,*p,s["heads"][i].as_u64().unwrap_or(9));}
  for(i,p)in origins.iter().enumerate(){write(&mut bytes,*p,row["origins"][i].as_u64().unwrap_or(2));}
  for p in &choices {write(&mut bytes,*p,0);}
  for(i,p)in token_codes.iter().enumerate(){write(&mut bytes,*p,pos.get(i).and_then(|x|x.as_u64()).unwrap_or(17));}
  // Existing fixture choices are zero; codes.0 carries each actual selected trajectory code.
  let fs=row["future"].as_array().unwrap();write(&mut bytes,future_count,fs.len() as u64);for(i,p)in future.iter().enumerate(){write(&mut bytes,*p,fs.get(i).and_then(|x|x.as_u64()).unwrap_or(17));}
  let actual=evaluator.evaluate(&bytes).unwrap();let view=validate_canonical_structured_value(actual).unwrap();let got=field(view,&["indices"]).collection_elements().unwrap().map(|x|u64::from_le_bytes(x.unwrap().value_node()[5..].try_into().unwrap())).collect::<Vec<_>>();let expected=row["expected"].as_array().unwrap().iter().map(|x|x.as_u64().unwrap()).collect::<Vec<_>>();assert_eq!(got,expected,"row {} state {} origins {}",row["id"],s,row["origin_policy"]);
  if total%1000==0{assert_eq!(values.evaluate(&bytes).unwrap(),actual,"allocating Source parity");}
  total+=1;
 }
 println!("PASS {total} exact original Source feature-value oracle vectors {:?}; synthetic structural contexts, not Native lexical admission/model inference",started.elapsed());
}
