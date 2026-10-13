use conduit_plot::{parse_syntax_document,check_syntax_document,StartupCatalog};
fn main(){
 let root=std::path::Path::new("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language");
 let source=["types","identity","coverage","syntax","text_revision","lexical","lexical_proposer","parser","parser_mask","parser_window8","parser_window8_search","parser_window8_facts"].iter().map(|n|std::fs::read_to_string(root.join(format!("{n}.conduit"))).unwrap()).collect::<Vec<_>>().join("\n");
 let source=source+"\n"+&std::fs::read_to_string("work/lexical-proposer-next/qualified_analysis.conduit").unwrap();
 let parsed=parse_syntax_document(&source);
 match check_syntax_document(&parsed,&StartupCatalog::new()) {Ok(c)=>println!("PASS checked_native_types={}",c.native_types.len()),Err(e)=>{eprintln!("{e:#?}");std::process::exit(1)}}
}
