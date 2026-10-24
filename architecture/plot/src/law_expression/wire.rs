//! Strict capsule framing; Source replay happens only with independent context.
use super::*;
const HEADER: &[u8] = b"conduit.law-expression.program.v1\0";
type Result<T> = core::result::Result<T, LawExpressionRefusal>;
impl LawExpressionMaterial {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        if self.roots.len() > 256 {
            return Err(LawExpressionRefusal::Capacity);
        }
        let program = self
            .program
            .canonical_bytes()
            .map_err(|_| LawExpressionRefusal::Encoding)?;
        let mut roots = self.roots.iter().collect::<Vec<_>>();
        roots.sort_by(|a, b| a.claimed.identity.cmp(&b.claimed.identity));
        if roots
            .windows(2)
            .any(|pair| pair[0].claimed.identity == pair[1].claimed.identity)
        {
            return Err(LawExpressionRefusal::Context);
        }
        let mut values = Vec::new();
        let mut total = HEADER.len() + 4 + LAW_EXPRESSION_PROFILE.len() + 4 + program.len() + 4;
        let mut documents = 0usize;
        for root in roots {
            let bytes = root
                .canonical_bytes()
                .map_err(LawExpressionRefusal::Custody)?;
            documents = documents
                .checked_add(
                    NativeLawMaterial::wire_document_count(&bytes)
                        .map_err(LawExpressionRefusal::Custody)?,
                )
                .ok_or(LawExpressionRefusal::Capacity)?;
            total = total
                .checked_add(4 + bytes.len())
                .ok_or(LawExpressionRefusal::Capacity)?;
            if documents > 256 || total > MAXIMUM_LAW_EXPRESSION_BYTES {
                return Err(LawExpressionRefusal::Capacity);
            }
            values.push(bytes);
        }
        if total > MAXIMUM_LAW_EXPRESSION_BYTES {
            return Err(LawExpressionRefusal::Capacity);
        }
        let mut encoded = Vec::with_capacity(total);
        encoded.extend_from_slice(HEADER);
        field(&mut encoded, LAW_EXPRESSION_PROFILE.as_bytes())?;
        field(&mut encoded, &program)?;
        encoded.extend_from_slice(&(values.len() as u32).to_le_bytes());
        for bytes in values {
            field(&mut encoded, &bytes)?;
        }
        Ok(encoded)
    }
    pub fn from_canonical_bytes(encoded: &[u8]) -> Result<Self> {
        if encoded.len() > MAXIMUM_LAW_EXPRESSION_BYTES {
            return Err(LawExpressionRefusal::Capacity);
        }
        let body = encoded
            .strip_prefix(HEADER)
            .ok_or(LawExpressionRefusal::Encoding)?;
        let mut cursor = Cursor(body);
        if cursor.field()? != LAW_EXPRESSION_PROFILE.as_bytes() {
            return Err(LawExpressionRefusal::Profile);
        }
        let program = cursor.field()?;
        let count = cursor.count()?;
        if count > 256 {
            return Err(LawExpressionRefusal::Capacity);
        }
        let mut frames = Vec::with_capacity(count);
        let mut documents = 0usize;
        for _ in 0..count {
            let frame = cursor.field()?;
            documents = documents
                .checked_add(
                    NativeLawMaterial::wire_document_count(frame)
                        .map_err(LawExpressionRefusal::Custody)?,
                )
                .ok_or(LawExpressionRefusal::Capacity)?;
            if documents > 256 {
                return Err(LawExpressionRefusal::Capacity);
            }
            frames.push(frame);
        }
        if !cursor.0.is_empty() {
            return Err(LawExpressionRefusal::Encoding);
        }
        // Every expanded Source/module/depth boundary is scanned above before
        // decoding any Type/predicate graph or replaying any Source.
        let program = PortableExpressionProgram::from_canonical_bytes(program)
            .map_err(|_| LawExpressionRefusal::Encoding)?;
        let roots = frames
            .into_iter()
            .map(|bytes| {
                NativeLawMaterial::from_canonical_bytes(bytes)
                    .map_err(LawExpressionRefusal::Custody)
            })
            .collect::<Result<Vec<_>>>()?;
        let material = Self { program, roots };
        if material.canonical_bytes()?.as_slice() != encoded {
            return Err(LawExpressionRefusal::Encoding);
        }
        Ok(material)
    }
}
fn field(output: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    output.extend_from_slice(
        &u32::try_from(bytes.len())
            .map_err(|_| LawExpressionRefusal::Capacity)?
            .to_le_bytes(),
    );
    output.extend_from_slice(bytes);
    Ok(())
}
struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let (value, remaining) = self
            .0
            .split_at_checked(n)
            .ok_or(LawExpressionRefusal::Encoding)?;
        self.0 = remaining;
        Ok(value)
    }
    fn count(&mut self) -> Result<usize> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| LawExpressionRefusal::Encoding)?,
        ) as usize)
    }
    fn field(&mut self) -> Result<&'a [u8]> {
        let n = self.count()?;
        self.take(n)
    }
}
