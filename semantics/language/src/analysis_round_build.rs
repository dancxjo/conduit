//! Canonical query construction for the one retained round machine.
use super::*;
impl Machine {
    fn scored(&mut self, mat: &mut Materializer, probe: bool) -> R<()> {
        let prior = if probe { &self.prior } else { &self.branch };
        mat.numeric(member(prior, &["parser", "state"])?, &mut self.numeric)?;
        if probe {
            mat.unsigned([0, 1, 2, 39, 73][self.probe], &mut self.scalar)?;
        }
        let score = if probe {
            view(&self.zero_i64)?
        } else {
            member(&self.rank, &["score"])?
        };
        mat.record(
            "LanguageParserScoredClass",
            &[
                ("basis", member(prior, &["parser", "state", "basis"])?),
                (
                    "class",
                    if probe {
                        view(&self.scalar)?
                    } else {
                        member(&self.rank, &["class"])?
                    },
                ),
                ("default_relation", view(&self.relation)?),
                ("score", score),
                ("state", view(&self.numeric)?),
            ],
            &mut self.query,
        )
    }
    pub fn build(&mut self, mat: &mut Materializer) -> R<Work> {
        use Phase::*;
        loop {
            match self.phase {
                Seed => {
                    put(&mut self.query, &self.initial_raw)?;
                    return Ok(Work::Source(
                        "language/analysis-round-empty",
                        "LanguageAnalysisInitialRawBeam",
                        Some("LanguageAnalysisInitialRawBeam"),
                    ));
                }
                SeedAdmit => {
                    let raw = view(&self.raw)?;
                    for (index, name) in ["candidate0", "candidate1", "candidate2", "candidate3"]
                        .into_iter()
                        .enumerate()
                    {
                        mat.hypothesis(field(raw, name)?, &mut self.slots[index])?;
                    }
                    mat.record(
                        "LanguageParserScorerBeam",
                        &[
                            ("candidate0", view(&self.slots[0])?),
                            ("candidate1", view(&self.slots[1])?),
                            ("candidate2", view(&self.slots[2])?),
                            ("candidate3", view(&self.slots[3])?),
                        ],
                        &mut self.query,
                    )?;
                    return Ok(Work::Native("LanguageParserScorerBeam"));
                }
                Slot => {
                    if self.slot == 4 {
                        self.phase = Done;
                        continue;
                    }
                    let name = ["candidate0", "candidate1", "candidate2", "candidate3"][self.slot];
                    let prior = member(&self.prior_beam, &[name])?;
                    if !boolean(field(field(prior, "parser")?, "active")?)? {
                        self.slot += 1;
                        continue;
                    }
                    copy_value(&mut self.prior, prior)?;
                    self.choice = 0;
                    self.probe = 0;
                    self.phase = Complete;
                }
                Complete => {
                    copy_value(&mut self.query, member(&self.prior, &["parser", "state"])?)?;
                    return Ok(Work::Source(
                        "language-parser-decode-complete",
                        "LanguageParserState",
                        None,
                    ));
                }
                ProbeProposal | RankedProposal => {
                    self.scored(mat, self.phase == ProbeProposal)?;
                    return Ok(Work::Source(
                        "language-parser-score-proposal",
                        "LanguageParserScoredClass",
                        Some("LanguageParserScoredProposal"),
                    ));
                }
                ProbeTransition | Transition => {
                    copy_value(&mut self.query, member(&self.proposal, &["request"])?)?;
                    return Ok(Work::Source(
                        "language-parser-transition",
                        "LanguageParserRequest",
                        Some("LanguageParserResult"),
                    ));
                }
                Mask => {
                    mat.record(
                        "LanguageParserScorerLegality",
                        &[
                            ("basis", member(&self.prior, &["parser", "state", "basis"])?),
                            ("left", view(&self.probes[2])?),
                            ("reduce", view(&self.probes[1])?),
                            ("right", view(&self.probes[3])?),
                            ("root", view(&self.probes[4])?),
                            ("shift", view(&self.probes[0])?),
                        ],
                        &mut self.query,
                    )?;
                    return Ok(Work::Source(
                        "language-parser-scorer-legality",
                        "LanguageParserScorerLegality",
                        Some("LanguageParserScorerRanking"),
                    ));
                }
                Choice => {
                    self.ranked = 0;
                    if self.alternative_count()? == 1 {
                        put(&mut self.branch, &self.prior)?;
                        self.phase = Score;
                        continue;
                    }
                    mat.unsigned(self.choice, &mut self.scalar)?;
                    mat.record(
                        "LanguageParserScorerBranch",
                        &[
                            ("choice", view(&self.scalar)?),
                            ("lexical", view(&self.lexical)?),
                            ("prior", view(&self.prior)?),
                        ],
                        &mut self.query,
                    )?;
                    self.phase = Branch;
                    return Ok(Work::Source(
                        "language-parser-scorer-branch",
                        "LanguageParserScorerBranch",
                        Some("LanguageParserScorerBranchResult"),
                    ));
                }
                BranchAdmit => {
                    mat.hypothesis(member(&self.raw, &["candidate"])?, &mut self.query)?;
                    return Ok(Work::Native("LanguageParserScorerHypothesis"));
                }
                Score => {
                    if self.mode == AnalysisRoundMode::AuthenticatedCategorical {
                        self.phase = Features;
                        continue;
                    }
                    put(&mut self.query, &self.mask)?;
                    return Ok(Work::Source(
                        "language-parser-scorer-fallback",
                        "LanguageParserScorerRanking",
                        Some("LanguageParserScorerRanking"),
                    ));
                }
                Features => {
                    mat.unsigned(self.invocation, &mut self.scalar)?;
                    mat.record(
                        "LanguageParserScorerChoiceQuery",
                        &[
                            ("choices", member(&self.branch, &["choices"])?),
                            (
                                "epoch",
                                member(&self.lexical, &["tape", "source", "sequence"])?,
                            ),
                            ("invocation", view(&self.scalar)?),
                            ("lexical", view(&self.lexical)?),
                            ("state", member(&self.branch, &["parser", "state"])?),
                        ],
                        &mut self.query,
                    )?;
                    put(&mut self.feature_query, &self.query)?;
                    return Ok(Work::Source(
                        "language-parser-scorer-model-features",
                        "LanguageParserScorerChoiceQuery",
                        Some("LanguageParserScorerFeatures"),
                    ));
                }
                Numerical => return Ok(Work::Numerical),
                LearnedRank => {
                    mat.record(
                        "LanguageParserScorerRanking",
                        &[
                            ("allowed", member(&self.mask, &["allowed"])?),
                            ("class", view(&self.zero)?),
                            ("cursor", view(&self.zero)?),
                            ("found", view(&self.false_value)?),
                            ("score", member(&self.mask, &["score"])?),
                            ("scores", view(&self.numerical_scores)?),
                        ],
                        &mut self.query,
                    )?;
                    return Ok(Work::Native("LanguageParserScorerRanking"));
                }
                Rank | NextRank => {
                    put(&mut self.query, &self.rank)?;
                    return Ok(Work::Source(
                        if self.phase == Rank {
                            "language-parser-scorer-rank"
                        } else {
                            "language-parser-scorer-ranking-next"
                        },
                        "LanguageParserScorerRanking",
                        Some("LanguageParserScorerRanking"),
                    ));
                }
                Expand => {
                    mat.unsigned(self.identity, &mut self.scalar)?;
                    mat.record(
                        "LanguageParserScorerExpansion",
                        &[
                            ("identity", view(&self.scalar)?),
                            ("prior", view(&self.branch)?),
                            ("result", view(&self.result)?),
                            ("score", member(&self.rank, &["score"])?),
                        ],
                        &mut self.query,
                    )?;
                    return Ok(Work::Source(
                        "language-parser-scorer-expand",
                        "LanguageParserScorerExpansion",
                        Some("LanguageParserScorerRawHypothesis"),
                    ));
                }
                CandidateAdmit => {
                    mat.hypothesis(view(&self.raw)?, &mut self.query)?;
                    return Ok(Work::Native("LanguageParserScorerHypothesis"));
                }
                Merge => {
                    mat.record(
                        "LanguageParserScorerMerge",
                        &[
                            ("beam", view(&self.next)?),
                            ("candidate", view(&self.candidate)?),
                        ],
                        &mut self.query,
                    )?;
                    return Ok(Work::Source(
                        "language-parser-scorer-merge",
                        "LanguageParserScorerMerge",
                        Some("LanguageParserScorerBeam"),
                    ));
                }
                Done => return Ok(Work::Complete),
                Branch => return Err(CursorRefusal::Lifecycle),
            }
        }
    }
}
