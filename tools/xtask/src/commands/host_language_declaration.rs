//! Explicit Host-owned Language metadata for one exact installed provider source.
use crate::cli::GlobalOpts;
use clap::Args;
use conduit_language::{
    LanguageCoverage, LanguageExternalIdentity, LanguageId, LanguageMappingDeclaration,
    LanguageRequest, LanguageVariety, LanguageVarietyPolicy, VarietyId,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_std_host::{
    hosted_speech_recognition::WhisperDiscovery, hosted_speech_synthesis::EspeakDiscovery,
};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

#[derive(Args, Debug)]
pub(super) struct Declaration {
    /// Exact portable Language identity declared by the Host owner; never inferred.
    #[arg(long)]
    language: String,
    /// Exact portable Variety identity; if supplied, this declaration requires it.
    #[arg(long)]
    variety: Option<String>,
    /// Native LanguageCoverage destination; must be a new file.
    #[arg(long)]
    output: PathBuf,
    /// Optional native LanguageRequest destination, with the same explicit semantic selection.
    #[arg(long)]
    request_output: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub(super) struct SpeechDeclaration {
    #[arg(long)]
    executable: PathBuf,
    #[arg(long)]
    data: PathBuf,
    #[arg(long, num_args = 1..)]
    engine: Vec<PathBuf>,
    #[arg(long)]
    voice: String,
    #[command(flatten)]
    declaration: Declaration,
}

#[derive(Args, Debug)]
pub(super) struct WhisperDeclaration {
    #[arg(long)]
    executable: PathBuf,
    #[arg(long)]
    model: PathBuf,
    /// Exact provider-private language option; no spelling-to-Language inference.
    #[arg(long)]
    provider_language: String,
    #[command(flatten)]
    declaration: Declaration,
}

impl Declaration {
    fn coverage(&self, artifact: &str, private_name: &str) -> Result<LanguageCoverage, String> {
        let language = LanguageId::new(self.language.clone())
            .map_err(|error| format!("Language identity: {error:?}"))?;
        let variety = self
            .variety
            .as_ref()
            .map(|identity| {
                LanguageVariety::new(VarietyId::new(identity.clone())?, language.clone())
            })
            .transpose()
            .map_err(|error| format!("Variety identity: {error:?}"))?;
        let row = LanguageMappingDeclaration::new(
            LanguageExternalIdentity::new(artifact.into(), private_name.into())
                .map_err(|error| format!("private identity: {error:?}"))?,
            language.clone(),
            variety.clone(),
        )
        .map_err(|error| format!("private mapping: {error:?}"))?;
        LanguageCoverage::new(
            artifact.into(),
            BoundedSequence::try_from_iter([language]).expect("one language"),
            BoundedSequence::try_from_iter([row]).expect("one mapping"),
            "host-declared@1".into(),
            BoundedSequence::try_from_iter(variety.clone()).expect("at most one variety"),
            variety.is_some(),
        )
        .map_err(|error| format!("coverage declaration: {error:?}"))
    }

    fn dry_run(&self, opts: &GlobalOpts) -> bool {
        if !opts.dry_run {
            return false;
        }
        if opts.json {
            println!(
                "{}",
                serde_json::json!({"schema":"conduit.tools/host-language-declaration@1", "dry_run":true,"effects_performed":false})
            );
        } else if !opts.quiet {
            println!("Would bind the explicitly declared Language and private mapping to the exact installed provider source; no provider execution.");
        }
        true
    }

    fn emit(
        &self,
        coverage: LanguageCoverage,
        opts: &GlobalOpts,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = coverage
            .clone()
            .encode()
            .map_err(|error| format!("encode Language coverage: {error:?}"))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.output)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        if let Some(path) = &self.request_output {
            let language = coverage.languages().as_slice()[0].clone();
            let variety = coverage.varieties().as_slice().first().cloned();
            let policy = if *coverage.variety_sensitive() {
                LanguageVarietyPolicy::ExactVariety
            } else {
                LanguageVarietyPolicy::LanguageSufficient
            };
            let request = LanguageRequest::new(language, variety, policy)
                .map_err(|error| format!("Language request: {error:?}"))?;
            let mut request_file = OpenOptions::new().write(true).create_new(true).open(path)?;
            request_file.write_all(
                &request
                    .encode()
                    .map_err(|error| format!("encode Language request: {error:?}"))?,
            )?;
            request_file.sync_all()?;
        }
        if opts.json {
            println!(
                "{}",
                serde_json::json!({"schema":"conduit.tools/host-language-declaration@1", "proof_class":"host-declared-metadata","artifact":coverage.evidence(),"language":self.language,"output":self.output,"provider_executed":false})
            );
        } else if !opts.quiet {
            println!("Retained artifact-bound Language declaration: {}. This records Host metadata; it does not prove speech accuracy.", self.output.display());
        }
        Ok(())
    }
}

pub(super) fn speech(
    request: SpeechDeclaration,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if request.declaration.dry_run(opts) {
        return Ok(());
    }
    let discovery = EspeakDiscovery::inspect(
        &request.executable,
        &request.data,
        &request.voice,
        &request.engine,
    )?;
    let coverage = request
        .declaration
        .coverage(&discovery.provider_identity(), &request.voice)?;
    discovery.declare_language_coverage(coverage.clone())?;
    request.declaration.emit(coverage, opts)
}

pub(super) fn whisper(
    request: WhisperDeclaration,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if request.declaration.dry_run(opts) {
        return Ok(());
    }
    let discovery = WhisperDiscovery::inspect(&request.executable, &request.model)?;
    let coverage = request
        .declaration
        .coverage(&discovery.provider_identity(), &request.provider_language)?;
    discovery.declare_language_coverage(coverage.clone())?;
    request.declaration.emit(coverage, opts)
}
