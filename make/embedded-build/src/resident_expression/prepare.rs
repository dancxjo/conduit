//! Cold construction and exact private-work sealing before artifact creation.
use super::*;
use sha2::{Digest, Sha256};

impl PreparedResidentExpression {
    pub fn prepare(
        document: &CheckedSyntaxDocument,
        program: &PortableExpressionProgram,
        temporal: c::PortTemporal,
        bounds: ResidentExpressionBounds,
    ) -> Result<Self, ResidentExpressionRefusal> {
        Self::prepare_instance(document, program, temporal, bounds, None)
    }
    /// Prepare an explicitly distinct initialized owner for shared checked code.
    /// The instance label describes cold construction; it grants no custody.
    pub fn prepare_instance(
        document: &CheckedSyntaxDocument,
        program: &PortableExpressionProgram,
        temporal: c::PortTemporal,
        bounds: ResidentExpressionBounds,
        instance: Option<&str>,
    ) -> Result<Self, ResidentExpressionRefusal> {
        Self::prepare_instance_with_storage(
            document,
            program,
            temporal,
            bounds,
            instance,
            ResidentExecutionStorage::Owned,
        )
    }
    /// Select explicit physical storage before sealing the emitted artifact.
    pub fn prepare_instance_with_storage(
        document: &CheckedSyntaxDocument,
        program: &PortableExpressionProgram,
        temporal: c::PortTemporal,
        bounds: ResidentExpressionBounds,
        instance: Option<&str>,
        storage: ResidentExecutionStorage,
    ) -> Result<Self, ResidentExpressionRefusal> {
        Self::prepare_with_work_selection(
            document, program, temporal, bounds, instance, storage, false,
        )
    }
    /// Seal the compiler's exact conservative work requirement. The supplied
    /// private-work bound is a ceiling, never an extra pool or execution grant.
    pub fn prepare_instance_with_exact_work(
        document: &CheckedSyntaxDocument,
        program: &PortableExpressionProgram,
        temporal: c::PortTemporal,
        bounds: ResidentExpressionBounds,
        instance: Option<&str>,
        storage: ResidentExecutionStorage,
    ) -> Result<Self, ResidentExpressionRefusal> {
        Self::prepare_with_work_selection(
            document, program, temporal, bounds, instance, storage, true,
        )
    }
    fn prepare_with_work_selection(
        document: &CheckedSyntaxDocument,
        program: &PortableExpressionProgram,
        temporal: c::PortTemporal,
        mut bounds: ResidentExpressionBounds,
        instance: Option<&str>,
        storage: ResidentExecutionStorage,
        exact_work: bool,
    ) -> Result<Self, ResidentExpressionRefusal> {
        if instance.is_some_and(|name| name.is_empty() || name.len() > 1024) {
            return Err(ResidentExpressionRefusal::Bounds);
        }
        use ResidentExpressionRefusal as R;
        if !matches!(
            temporal,
            c::PortTemporal::Value | c::PortTemporal::Flow { closes: true }
        ) {
            return Err(R::Contract);
        }
        if bounds.private_work_units == 0 || bounds.private_work_units > u16::MAX - 3 {
            return Err(R::Bounds);
        }
        let binary = program.canonical_bytes().map_err(|_| R::Program)?;
        if binary.len() as u64 > u64::from(bounds.maximum_program_bytes) {
            return Err(R::ProgramBytes {
                required: binary.len(),
                maximum: bounds.maximum_program_bytes,
            });
        }
        let definition = conduit_plot::portable_expression_definition(program, temporal)
            .map_err(|_| R::Contract)?;
        let generated = conduit_plot::generate_static_structured_expression_with_work(
            document,
            program,
            bounds.native,
            bounds.maximum_occurrences,
        )
        .map_err(R::Generation)?;
        if generated.computation_work > u32::from(bounds.private_work_units) {
            return Err(R::PrivateWork {
                required: generated.computation_work,
                maximum: bounds.private_work_units,
            });
        }
        let computation_work = u16::try_from(generated.computation_work).map_err(|_| R::Bounds)?;
        if exact_work {
            bounds.private_work_units = computation_work;
        }
        let mut code = generated.source;
        let maximum_input = program
            .maximum_prepared_input_bytes()
            .map_err(|_| R::Bounds)?;
        let temporal_rust = match temporal {
            c::PortTemporal::Value => "conduit_core::PortTemporal::Value",
            _ => "conduit_core::PortTemporal::Flow {closes:true}",
        };
        let output = program
            .maximum_prepared_output_bytes()
            .map_err(|_| R::Bounds)?;
        crate::resident_back::append(&mut code, output, temporal_rust, storage);
        code.push_str(&format!(
            "pub const PRIVATE_WORK_UNITS:u16={};\n",
            bounds.private_work_units
        ));
        if code.len() as u64 > u64::from(bounds.maximum_generated_bytes) {
            return Err(R::GeneratedBytes {
                required: code.len(),
                maximum: bounds.maximum_generated_bytes,
            });
        }
        if let Some(instance) = instance {
            let digest: [u8; 32] = Sha256::digest(instance.as_bytes()).into();
            // The instance marker belongs to the generated realization, not the
            // pure program's Kind. Assigned-node artifact pins distinguish the
            // initialized owners even though they execute identical code.
            code.push_str(&format!(
                "pub const INITIALIZED_INSTANCE:[u8;32]={digest:?};\n"
            ));
        }
        let digest = Sha256::digest(code.as_bytes());
        let artifact = format!("conduit.expression/resident-source@1/{digest:x}").into();
        let offer = crate::resident_offer::append(
            &mut code,
            program,
            temporal,
            &artifact,
            bounds.private_work_units,
            instance,
        )?;
        if code.len() as u64 > u64::from(bounds.maximum_generated_bytes) {
            return Err(R::GeneratedBytes {
                required: code.len(),
                maximum: bounds.maximum_generated_bytes,
            });
        }
        Ok(Self {
            storage,
            source_document: document.source_document_id.clone(),
            maximum_input,
            maximum_output: output,
            maximum_generated_bytes: bounds.maximum_generated_bytes,
            computation_work,
            text: program.canonical_hex().map_err(|_| R::Program)?,
            temporal,
            code,
            artifact,
            kind: definition.kind_id,
            offer,
        })
    }
}
