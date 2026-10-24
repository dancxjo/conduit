# Pinned TRAIN lexical resource

Provenance: Universal Dependencies English EWT, commit `b7711cce01cdd4f5fcc0a8199b8a50d951b16c0c`, TRAIN partition only. The original corpus license and contributor README are retained beside this file. No raw corpus, derived binary or trained model is included here.

The exact TRAIN input has 15,034,493 bytes and SHA256 `d68e06122a702464c613076523d56740f047e5bbe89dd90ec32737e04d952143`. The finite `CLXPOS1` resource groups exact-case UTF-8 surfaces with their sorted observed UPOS code sets. It retains only nonempty surfaces of at most 128 bytes with at most 4 candidate classes. It excludes multiword-token and empty-node rows. It does not select or infer a POS.

Reconstruction produces 19,652 rows and 222,999 bytes, SHA256 `05c48c7e2e02e68088b2e359808a6fde72a1713aa8b2c1f0bf025f4583e0467f`. The content semantic digest and raw SHA256 use distinct contracts; neither is a read grant. Core resource/read admission and exact retained bytes remain required.

The source pin and finite profile are part of #6072. Native preparation authenticates complete rows against the admitted pinned resource and prepares a table of at most four distinct current-token surfaces. The checked Source lookup searches this exact bounded table, returning its row index (0..3) or explicit absent value4. Source correlation and revision rebase preserve the entire ordered candidate row and token lineage. This lookup does not read or search the entire resource during Play.

Run the supported check with an existing pinned corpus file:

```sh
cargo +stable xtask check resource-epochs --train-conllu /path/to/en_ewt-ud-train.conllu
```

The previous consuming-owner source passed all41 supported tests, but its Boolean Source correlation alone did not implement the literal Source lookup requirement. The added actual lookup passes both focused Source tests and the final supported run passes all 42 tests, with strict owning checks and formatting. Exact-head Candidate/Integration and shared acceptance remain pending. No broad lexical coverage, learned accuracy, installed resource-reader Back or streaming decoder is claimed.
