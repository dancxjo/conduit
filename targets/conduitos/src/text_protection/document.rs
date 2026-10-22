//! Canonical revisions over the existing protected retained text owner.
use super::*;
use conduit_semantic_catalog::{TextDocument, TextEdit, TextEditRefusal};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProtectedDocumentError {
    Refused(TextEditRefusal),
    Execution(MachineRunError),
}

/// One bounded observation, not another editable document store.
pub(crate) struct ProtectedDocument {
    bytes: [u8; conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES as usize],
    length: usize,
}
impl ProtectedDocument {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    pub fn document(&self) -> TextDocument<'_> {
        TextDocument::decode(self.as_bytes()).expect("checked protected document observation")
    }
}

impl<I: TextOwner> ProtectedText<I> {
    pub fn read_document(&mut self) -> Result<ProtectedDocument, ProtectedDocumentError> {
        self.document_exchange(0, &[])
    }

    pub fn replace_document(
        &mut self,
        edit: TextEdit<'_>,
    ) -> Result<ProtectedDocument, ProtectedDocumentError> {
        let mut bytes = [0; conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES as usize];
        let length = edit
            .encode_into(&mut bytes)
            .map_err(ProtectedDocumentError::Refused)?;
        let expected = edit.base_revision.checked_add(1);
        let observed = self.document_exchange(1, &bytes[..length])?;
        if Some(observed.document().revision) != expected {
            self.region.fault(
                crate::protected_region::DomainFault::InvalidGate,
                &mut self.capabilities,
            );
            return Err(ProtectedDocumentError::Execution(
                MachineRunError::ProtectionFault(crate::protected_region::DomainFault::InvalidGate),
            ));
        }
        Ok(observed)
    }

    fn document_exchange(
        &mut self,
        operation: u32,
        input: &[u8],
    ) -> Result<ProtectedDocument, ProtectedDocumentError> {
        if !self.editor {
            return Err(ProtectedDocumentError::Refused(
                TextEditRefusal::UnsupportedMode,
            ));
        }
        let backend = self.region.backend_mut().map_err(|error| {
            ProtectedDocumentError::Execution(MachineRunError::ProtectionDomain(error))
        })?;
        backend.document_input(operation, input).map_err(|error| {
            ProtectedDocumentError::Execution(MachineRunError::ProtectionDomain(error))
        })?;
        self.return_from_pure()
            .map_err(ProtectedDocumentError::Execution)?;
        let mut observed = ProtectedDocument {
            bytes: [0; conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES as usize],
            length: 0,
        };
        let result = self
            .region
            .backend_mut()
            .map_err(|error| {
                ProtectedDocumentError::Execution(MachineRunError::ProtectionDomain(error))
            })?
            .document_output(operation, &mut observed.bytes);
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                self.region.fault(
                    crate::protected_region::DomainFault::InvalidGate,
                    &mut self.capabilities,
                );
                return Err(ProtectedDocumentError::Execution(
                    MachineRunError::ProtectionDomain(error),
                ));
            }
        };
        observed.length = result.map_err(ProtectedDocumentError::Refused)?;
        Ok(observed)
    }
}
