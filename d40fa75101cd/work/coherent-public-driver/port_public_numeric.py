from pathlib import Path
import re, shutil, os
repo=Path(__file__).resolve().parents[2];w=repo/'work/coherent-public-driver';src=w/'numeric-owner';dest=repo/'semantics/ai/src';old=Path('/home/dancxjo/conduit-4898-common-greeting/semantics/ai');project=Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907/work/fargan/public-model-driver')
for p in src.rglob('*.rs'):
 if p.name=='lib.rs':continue
 rel=p.relative_to(src);q=dest/rel
 if q.exists():raise RuntimeError(('refuse existing source overwrite',q))
 q.parent.mkdir(parents=True,exist_ok=True);s=p.read_text()
 s=re.sub(r'(?m)^#\[path="'+re.escape(str(src))+r'/[^"\n]+"\]\n','',s)
 for match in list(re.finditer(r'include_str!\("('+re.escape(str(old))+r'/[^"\n]+)"\)',s)):
  source=Path(match[1]);target=repo/'semantics/ai'/source.relative_to(old)
  if not target.exists():target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,target)
  s=s.replace(match[0],'include_str!("'+os.path.relpath(target,q.parent)+'")')
 s=s.replace(f'#[path="{project}/native_profile_storage.rs"]\nmod bounded_storage;\npub use bounded_storage::*;','mod storage;\npub use storage::*;')
 s=s.replace(f'#[path="{project}/native_factory_storage.rs"]\nmod bounded_storage;\npub use bounded_storage::*;','mod storage;\npub use storage::*;')
 s=s.replace(f'#[path="{project}/native_traversal_storage.rs"]\nmod bounded_storage;\npub use bounded_storage::*;','mod storage;\npub use storage::*;')
 s=s.replace(f'#[path="{w}/numeric-storage-unit-tests.rs"]\n','')
 q.write_text(s)
for rel,source in [('native_profile/storage.rs','native_profile_storage.rs'),('operation_owners/native_profile/storage.rs','native_factory_storage.rs'),('native_traversal/storage.rs','native_traversal_storage.rs'),('transport_envelope.rs','transport_envelope.rs')]:
 q=dest/rel;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes((project/source).read_bytes())
q=dest/'native_profile/storage_tests.rs';q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes((w/'numeric-storage-unit-tests.rs').read_bytes())
p=dest/'transport_envelope.rs';p.write_text(p.read_text().replace('use super::*;use conduit_core::{kind_id,StructuredFieldType};','use super::*;use alloc::vec;use conduit_core::{kind_id,StructuredFieldType};'))
mods=re.findall(r'pub mod (\w+);',(src/'lib.rs').read_text());p=dest/'lib.rs';s=p.read_text();s+='\n// Existing authored fixed numerical profiles; explicit opt-in keeps the generic\n// categorical owner independent of this supported numerical capability.\n';s+=''.join(f'#[cfg(feature = "fixed-numeric-owners")]\npub mod {name};\n' for name in mods)
s+='\n#[cfg(target_has_atomic = "ptr")]\nmod model_compute_owned;\n#[cfg(target_has_atomic = "ptr")]\npub use model_compute_owned::*;\n'
s+='\n#[cfg(all(test, feature = "fixed-numeric-owners"))]\nextern crate std;\n#[cfg(all(test, feature = "fixed-numeric-owners"))]\n#[path="numeric_allocation_probe.rs"]\nmod allocation_probe;\n#[cfg(all(test, feature = "fixed-numeric-owners"))]\n#[global_allocator]\nstatic NUMERIC_ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;\n';p.write_text(s)
(dest/'numeric_allocation_probe.rs').write_bytes((w/'allocation_probe.rs').read_bytes());(dest/'model_compute_owned.rs').write_bytes((old/'src/model_compute_owned.rs').read_bytes())
p=repo/'semantics/ai/Cargo.toml';s=p.read_text().replace('[features]\n','[features]\nfixed-numeric-owners = ["kernel-operation-owners"]\n');p.write_text(s)
print('ported',len(mods),'public modules; existing authored Source preserved')
