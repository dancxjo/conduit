# Finite jointly supervised ranking artifact

This separate artifact uses explicit wrong-UPOS contexts during averaged structured perceptron training. The previous conditional gold-UPOS artifact is preserved. Training compares a gold (feature context, action label) with every profile-permitted POS context and action label at gold oracle states. Source Plots still own feature encoding, POS choice projection, legal actions, transitions and beam retention.

The 64 exact FORM entries are selected only from eligible <=4-token official TRAIN sentences, ordered by descending frequency then text, excluding entries with more than four TRAIN-observed UPOS alternatives. Official dev/test splits do not select entries or alternatives. Annotations contain licensed derived FORM/POS/dependency data; original corpus files remain external. Manifest records the pinned official UD release/commit, license, corpus digests, all corpus/profile exclusions, exact weights/profile/Source identities and precision.

Reproduce with `python3 semantics/language/training/train_ewt_joint.py --corpus PATH_TO_OFFICIAL_EWT_2_18 --output OUTPUT`. Python is the external training/oracle diagnostic tool, not runtime inference or grammar authority.

Manifest lexical diagnostics condition on gold action sequences; joint context/action diagnostics condition on gold parser states. Neither is blind native lexical/attachment accuracy. Native held-out decoding and canonical lexer exclusions must be reported separately. Raw integer ranking scores do not represent calibrated lexical confidence. The profile is finite, exact-case and at most four tokens; it does not establish broad English accuracy.
