//! Canonical transport of untrusted law claims, never a witness constructor.
use super::*;
use crate::{NativeTypeValueContract, PortableExpressionProgram};

const HEADER: &[u8] = b"conduit.native-law-material.v1\0";
// A conservative transport cap, separate from and no larger than custody.
const MAXIMUM_WIRE_BYTES: usize = MAXIMUM_BYTES;

impl NativeLawMaterial {
    /// Internal aggregate framing preflight for a complete expression capsule.
    /// The caller sums this count across every expanded material before replay.
    pub(crate) fn wire_document_count(bytes: &[u8]) -> Result<usize> {
        if bytes.len() > MAXIMUM_WIRE_BYTES {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        let body = bytes
            .strip_prefix(HEADER)
            .ok_or(NativeLawCustodyRefusal::Source)?;
        let mut cursor = Cursor {
            remaining: body,
            modules: 0,
        };
        let _ = cursor.material(0, false)?;
        if !cursor.remaining.is_empty() {
            return Err(NativeLawCustodyRefusal::Source);
        }
        Ok(cursor.modules)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        material_preflight::check(self)?;
        let mut size = HEADER.len();
        encode(self, &mut |chunk| {
            size = size
                .checked_add(chunk.len())
                .ok_or(NativeLawCustodyRefusal::Capacity)?;
            if size > MAXIMUM_WIRE_BYTES {
                return Err(NativeLawCustodyRefusal::Capacity);
            }
            Ok(())
        })?;
        let mut bytes = Vec::with_capacity(size);
        bytes.extend_from_slice(HEADER);
        encode(self, &mut |chunk| {
            bytes.extend_from_slice(chunk);
            Ok(())
        })?;
        Ok(bytes)
    }

    /// Decode only an untrusted claim. Call `NativeLawWitness::verify_with_context`
    /// with independent expected Type/family anchors before using its laws.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAXIMUM_WIRE_BYTES {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        let body = bytes
            .strip_prefix(HEADER)
            .ok_or(NativeLawCustodyRefusal::Source)?;
        // Scan all framing/count/depth limits before any decoded graph allocation.
        let mut scan = Cursor {
            remaining: body,
            modules: 0,
        };
        let _ = scan.material(0, false)?;
        if !scan.remaining.is_empty() {
            return Err(NativeLawCustodyRefusal::Source);
        }
        let mut cursor = Cursor {
            remaining: body,
            modules: 0,
        };
        let value = cursor
            .material(0, true)?
            .ok_or(NativeLawCustodyRefusal::Source)?;
        material_preflight::check(&value)?;
        if value.canonical_bytes()?.as_slice() != bytes {
            return Err(NativeLawCustodyRefusal::Source);
        }
        Ok(value)
    }
}

type Result<T> = core::result::Result<T, NativeLawCustodyRefusal>;
type Dependencies = (
    Vec<(String, NativeLawMaterial)>,
    Vec<(String, NativeFamilyMaterial)>,
);
fn count(value: usize, sink: &mut impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
    sink(
        &u32::try_from(value)
            .map_err(|_| NativeLawCustodyRefusal::Capacity)?
            .to_le_bytes(),
    )
}
fn field(bytes: &[u8], sink: &mut impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
    count(bytes.len(), sink)?;
    sink(bytes)
}
fn encode(value: &NativeLawMaterial, sink: &mut impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
    field(value.source.as_bytes(), sink)?;
    field(value.declaration.as_bytes(), sink)?;
    field(value.claimed.name.as_bytes(), sink)?;
    field(value.claimed.identity.as_str().as_bytes(), sink)?;
    field(
        &value
            .claimed
            .value_type
            .canonical_bytes()
            .map_err(|_| NativeLawCustodyRefusal::Type)?,
        sink,
    )?;
    count(value.claimed.value_contracts.len(), sink)?;
    for contract in &value.claimed.value_contracts {
        field(contract.representation_path.as_bytes(), sink)?;
        field(&contract.contract.identity_bytes(), sink)?;
    }
    count(value.claimed.invariants.len(), sink)?;
    for program in &value.claimed.invariants {
        field(
            &program
                .canonical_bytes()
                .map_err(|_| NativeLawCustodyRefusal::Laws)?,
            sink,
        )?;
    }
    dependencies(&value.imports, &value.families, sink)
}
fn dependencies(
    imports: &[(String, NativeLawMaterial)],
    families: &[(String, NativeFamilyMaterial)],
    sink: &mut impl FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    count(imports.len(), sink)?;
    for (name, value) in imports {
        field(name.as_bytes(), sink)?;
        encode(value, sink)?;
    }
    count(families.len(), sink)?;
    for (name, value) in families {
        field(name.as_bytes(), sink)?;
        encode_family(value, sink)?;
    }
    Ok(())
}
fn encode_family(
    value: &NativeFamilyMaterial,
    sink: &mut impl FnMut(&[u8]) -> Result<()>,
) -> Result<()> {
    field(value.manifest.as_bytes(), sink)?;
    count(value.members.len(), sink)?;
    for member in &value.members {
        field(member.path.as_bytes(), sink)?;
        field(member.source.as_bytes(), sink)?;
    }
    field(value.exported_path.as_bytes(), sink)?;
    dependencies(&value.imports, &value.families, sink)
}

struct Cursor<'a> {
    remaining: &'a [u8],
    modules: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let (value, remaining) = self
            .remaining
            .split_at_checked(length)
            .ok_or(NativeLawCustodyRefusal::Source)?;
        self.remaining = remaining;
        Ok(value)
    }
    fn count(&mut self, maximum: usize) -> Result<usize> {
        let bytes = self.take(4)?;
        let value = u32::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| NativeLawCustodyRefusal::Source)?,
        ) as usize;
        if value > maximum {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        Ok(value)
    }
    fn field(&mut self, maximum: usize) -> Result<&'a [u8]> {
        let length = self.count(maximum)?;
        self.take(length)
    }
    fn text(&mut self, maximum: usize, decode: bool) -> Result<String> {
        let text = core::str::from_utf8(self.field(maximum)?)
            .map_err(|_| NativeLawCustodyRefusal::Source)?;
        Ok(if decode { text.into() } else { String::new() })
    }
    fn module(&mut self, depth: usize) -> Result<()> {
        self.modules = self
            .modules
            .checked_add(1)
            .ok_or(NativeLawCustodyRefusal::Capacity)?;
        if depth > MAXIMUM_DEPTH || self.modules > MAXIMUM_MODULES {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        Ok(())
    }
    fn material(&mut self, depth: usize, decode: bool) -> Result<Option<NativeLawMaterial>> {
        self.module(depth)?;
        let source = self.text(MAXIMUM_BYTES, decode)?;
        let declaration = self.text(4096, decode)?;
        let name = self.text(MAXIMUM_BYTES, decode)?;
        let identity = self.text(MAXIMUM_BYTES, decode)?;
        let type_bytes = self.field(MAXIMUM_BYTES)?;
        let contracts = self.count(4096)?;
        let mut value_contracts = Vec::new();
        for _ in 0..contracts {
            let representation_path = self.text(MAXIMUM_BYTES, decode)?;
            let bytes = self.field(MAXIMUM_BYTES)?;
            if decode {
                value_contracts.push(NativeTypeValueContract {
                    representation_path,
                    contract: conduit_core::CheckedValueContract::from_identity_bytes(bytes)
                        .map_err(|_| NativeLawCustodyRefusal::Laws)?,
                });
            }
        }
        let laws = self.count(4096)?;
        let mut invariants = Vec::new();
        for _ in 0..laws {
            let bytes = self.field(MAXIMUM_BYTES)?;
            if decode {
                invariants.push(
                    PortableExpressionProgram::from_canonical_bytes(bytes)
                        .map_err(|_| NativeLawCustodyRefusal::Laws)?,
                );
            }
        }
        let (imports, families) = self.dependencies(depth, decode)?;
        if !decode {
            return Ok(None);
        }
        Ok(Some(NativeLawMaterial {
            source,
            declaration,
            imports,
            families,
            claimed: CheckedNativeType {
                name,
                identity: conduit_core::kind_id(&identity),
                value_type: StructuredInfoType::from_canonical_bytes(type_bytes)
                    .map_err(|_| NativeLawCustodyRefusal::Type)?,
                value_contracts,
                invariants,
            },
        }))
    }
    fn dependencies(&mut self, depth: usize, decode: bool) -> Result<Dependencies> {
        let count = self.count(MAXIMUM_MODULES)?;
        let mut imports = Vec::new();
        for _ in 0..count {
            let name = self.text(4096, decode)?;
            if let Some(value) = self.material(depth + 1, decode)? {
                imports.push((name, value));
            }
        }
        let count = self.count(MAXIMUM_MODULES)?;
        let mut families = Vec::new();
        for _ in 0..count {
            let name = self.text(4096, decode)?;
            if let Some(value) = self.family(depth + 1, decode)? {
                families.push((name, value));
            }
        }
        Ok((imports, families))
    }
    fn family(&mut self, depth: usize, decode: bool) -> Result<Option<NativeFamilyMaterial>> {
        self.module(depth)?;
        let manifest = self.text(MAXIMUM_BYTES, decode)?;
        let count = self.count(crate::MAXIMUM_PACKAGE_MEMBERS)?;
        let mut members = Vec::new();
        for _ in 0..count {
            // Count each Source member against the existing document budget.
            self.module(depth)?;
            let path = self.text(4096, decode)?;
            let source = self.text(MAXIMUM_BYTES, decode)?;
            if decode {
                members.push(NativeFamilyMember { path, source });
            }
        }
        let exported_path = self.text(4096, decode)?;
        let (imports, families) = self.dependencies(depth, decode)?;
        Ok(decode.then_some(NativeFamilyMaterial {
            manifest,
            members,
            exported_path,
            imports,
            families,
        }))
    }
}
