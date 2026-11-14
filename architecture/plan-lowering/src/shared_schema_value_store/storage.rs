use super::*;
impl<S: ValueStorage> ValueStorage for SharedSchemaValueStore<'_, S> {
    fn item_capacity(&self) -> u16 {
        self.canonical_slots + self.logical.len() as u16
    }
    fn byte_capacity(&self) -> u32 {
        self.canonical.byte_capacity() + self.byte_capacity
    }
    fn used_items(&self) -> u16 {
        self.canonical.used_items() + self.used_items
    }
    fn used_bytes(&self) -> u32 {
        self.canonical.used_bytes() + self.used_bytes
    }
    fn preflight_canonical_store(&self, len: u32) -> Result<(), E> {
        self.canonical.preflight_canonical_store(len)
    }
    fn store(&mut self, bytes: &[u8]) -> Result<ValueRef, E> {
        self.canonical.store(bytes)
    }
    fn supports_schema_storage(&self, schema: &[u8]) -> bool {
        self.classes.iter().any(|c| c.type_bytes() == schema)
    }
    fn store_schema_canonical(&mut self, schema: &[u8], bytes: &[u8]) -> Result<ValueRef, E> {
        let validator = self
            .classes
            .iter()
            .find(|c| c.type_bytes() == schema && bytes.len() <= c.maximum_bytes())
            .copied()
            .ok_or(E::RepresentationMismatch)?;
        let value = validator
            .validate(bytes)
            .map_err(|_| E::RepresentationMismatch)?;
        if value.type_bytes() != schema {
            return Err(E::RepresentationMismatch);
        }
        self.store_structured(value)
    }
    fn get(&self, value: ValueRef) -> Result<&[u8], E> {
        if self.index(value).is_some() {
            self.resolve(value)?;
            Err(E::RepresentationMismatch)
        } else {
            self.canonical.get(value)
        }
    }
    fn canonical_parts(&self, value: ValueRef) -> Result<Option<CanonicalValueParts<'_>>, E> {
        if self.index(value).is_none() {
            return self.canonical.canonical_parts(value);
        }
        let (i, j) = self.resolve(value)?;
        Ok(Some(CanonicalValueParts {
            schema: self.classes[usize::from(self.logical[i].class)].type_bytes(),
            node: &self.physical[j].node,
        }))
    }
    fn reference_count(&self, value: ValueRef) -> Result<u16, E> {
        if self.index(value).is_some() {
            let (i, _) = self.resolve(value)?;
            Ok(self.logical[i].references)
        } else {
            self.canonical.reference_count(value)
        }
    }
    fn retain(&mut self, value: ValueRef) -> Result<(), E> {
        if self.index(value).is_some() {
            let (i, _) = self.resolve(value)?;
            self.logical[i].references = self.logical[i]
                .references
                .checked_add(1)
                .ok_or(E::ReferenceOverflow)?;
            Ok(())
        } else {
            self.canonical.retain(value)
        }
    }
    fn release(&mut self, value: ValueRef) -> Result<(), E> {
        if self.index(value).is_some() {
            let (i, j) = self.resolve(value)?;
            let slot = &mut self.logical[i];
            slot.references -= 1;
            if slot.references == 0 {
                slot.buffer = None;
                self.physical[j].owner = None;
                self.physical[j].node.clear();
                self.used_items -= 1;
                self.used_bytes -= value.byte_len;
            }
            Ok(())
        } else {
            self.canonical.release(value)
        }
    }
    fn clear(&mut self) {
        self.canonical.clear();
        for slot in &mut self.logical {
            slot.references = 0;
            slot.buffer = None;
        }
        for buffer in &mut self.physical {
            buffer.owner = None;
            buffer.node.clear();
        }
        self.used_items = 0;
        self.used_bytes = 0;
    }
}
