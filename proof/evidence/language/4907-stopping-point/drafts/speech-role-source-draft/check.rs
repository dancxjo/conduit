use conduit_plot::*;
use std::{collections::BTreeSet,path::Path};
#[path="/home/dancxjo/conduit-4907-production-custody/semantics/language/build_session_chain.rs"] mod chain;
fn main() {
 let dir=std::env::args().nth(1).unwrap();let dir=Path::new(&dir);
 let source=std::fs::read_to_string(dir.join("complete-original-input.conduit")).unwrap();
 let mut parsed=parse_syntax_document(&source);assert!(parsed.diagnostics.is_empty(),"{:?}",parsed.diagnostics);
 let mut names=BTreeSet::from(["LanguageParserWindow8CommittedDependencyProposal".to_string(),"LanguageParserWindow8CommittedDependencyAdmission".to_string(),"SpeechTextTokenRoleRequest".to_string(),"SpeechTextTokenRoleResult".to_string()]);
 loop {let before=names.len();for ty in &parsed.types {if names.contains(&ty.name.text) {let material=&source[ty.span.start..ty.span.end];for token in material.split(|c:char|!c.is_ascii_alphanumeric()&&c!='_') {if parsed.types.iter().any(|t|t.name.text==token) {names.insert(token.to_string());}}}}if names.len()==before{break}}
 parsed.types.retain(|t|names.contains(&t.name.text));
 parsed.plots.retain(|p|p.name.text=="language-window8-committed-dependency-role"||p.name.text=="speech/text-token-role");
 parsed.type_forms.clear();parsed.constructions.clear();parsed.packages.clear();
 std::fs::write(dir.join("selected-original-type-names.txt"),names.iter().cloned().collect::<Vec<_>>().join("\n")).unwrap();
 let checked=check_syntax_document(&parsed,&StartupCatalog::new()).unwrap();
 for name in ["language-window8-committed-dependency-role","speech/text-token-role"] {
 let expanded=expand_canonical_plot_for_authoring(&checked,name,&ProfileCatalog::new()).unwrap();chain::retain(&expanded,&dir.join(name.replace('/',"-")));println!("checked Source {name} gears={}",expanded.expanded.gears.len());}
 for ty in &checked.native_types {if ty.name.starts_with("LanguageParserWindow8CommittedDependency")||ty.name.starts_with("SpeechTextTokenRole") {std::fs::write(dir.join(format!("{}.type.bin",ty.name)),ty.value_type.canonical_bytes().unwrap()).unwrap();for (i,law) in ty.invariants.iter().enumerate(){std::fs::write(dir.join(format!("{}.law{i}.hex",ty.name)),law.canonical_hex().unwrap()).unwrap();}println!("type={} bytes={} laws={}",ty.name,ty.value_type.canonical_bytes().unwrap().len(),ty.invariants.len());}}
 println!("complete original Native declarations checked={}",checked.native_types.len());
}
