# Frozen parser comparison

Complete, finite, nonblind replacement diagnostic: 17 frozen histories in each arm. The original 29-report run and failed 28-report replacement are excluded. Original source-pin equivalence remains unestablished. No model, default or installed Current acceptance is claimed.

| Observation | Causal arm | Full arm |
| --- | ---: | ---: |
| Tokens | 47 | 47 |
| Correct unlabeled attachments | 36 | 36 |
| Correct labeled attachments | 36 | 36 |
| Vocative true positives | 4 | 4 |
| Vocative false positives | 0 | 0 |
| Vocative false negatives | 4 | 4 |
| Cases with prefinal commitment | 1 | 1 |
| Accepted commitment observations | 7 | 7 |
| Observed changes after commitment | 0 | 0 |
| Winner edge revisions | 13 | 13 |
| Numerical model invocations | 627 | 622 |
| Illegal candidates refused | 1810 | 1793 |

## Paired history outcomes

| History | Causal UAS/LAS | Full UAS/LAS | Causal vocative TP/FP/FN | Full vocative TP/FP/FN | Causal/full prefinal commitment |
| --- | --- | --- | --- | --- | --- |
| email-enronsent29_01-0077 | 2/2; 2/2 | 2/2; 2/2 | 0/0/0 | 0/0/0 | False/False |
| email-enronsent19_02-0045 | 2/2; 2/2 | 2/2; 2/2 | 0/0/0 | 0/0/0 | False/False |
| email-enronsent00_02-0052 | 2/2; 2/2 | 2/2; 2/2 | 0/0/0 | 0/0/0 | False/False |
| reviews-356361-0001 | 2/2; 2/2 | 2/2; 2/2 | 0/0/0 | 0/0/0 | False/False |
| reviews-371492-0001 | 3/3; 3/3 | 3/3; 3/3 | 0/0/0 | 0/0/0 | False/False |
| email-enronsent27_02-0013 | 3/3; 3/3 | 3/3; 3/3 | 0/0/0 | 0/0/0 | False/False |
| email-enronsent04_02-0007 | 3/3; 3/3 | 3/3; 3/3 | 0/0/0 | 0/0/0 | False/False |
| email-enronsent36_01-0001 | 2/2; 2/2 | 2/2; 2/2 | 0/0/0 | 0/0/0 | False/False |
| email-enronsent36_01-0007 | 2/2; 2/2 | 2/2; 2/2 | 0/0/0 | 0/0/0 | False/False |
| email-enronsent36_01-0031 | 0/3; 0/3 | 0/3; 0/3 | 0/0/1 | 0/0/1 | False/False |
| reserved-punctuation-evaluation/motivator-partial | 3/3; 3/3 | 3/3; 3/3 | 1/0/0 | 1/0/0 | False/False |
| reserved-punctuation-evaluation/motivator-period | 4/4; 4/4 | 4/4; 4/4 | 1/0/0 | 1/0/0 | True/True |
| reserved-punctuation-evaluation/reserved-name | 3/3; 3/3 | 3/3; 3/3 | 1/0/0 | 1/0/0 | False/False |
| reserved-punctuation-evaluation/reserved-greeting | 3/3; 3/3 | 3/3; 3/3 | 1/0/0 | 1/0/0 | False/False |
| reserved-punctuation-evaluation/reserved-initial | 1/3; 1/3 | 1/3; 1/3 | 0/0/1 | 0/0/1 | False/False |
| reserved-punctuation-evaluation/reserved-terminal | 0/3; 0/3 | 0/3; 0/3 | 0/0/1 | 0/0/1 | False/False |
| reserved-punctuation-evaluation/reserved-medial | 1/4; 1/4 | 1/4; 1/4 | 0/0/1 | 0/0/1 | False/False |

## Time and resource boundary

The whole worker recorded 21052.570 seconds wall time, 20957.729 seconds child user CPU and 51.153 seconds child system CPU. Maximum child RSS was 1207656 KiB. These are whole-process observations on this machine, not per-arm portable resource guarantees.

The retained canonical Source observations and preparation checks add diagnostic work. No real-time or broad accuracy claim follows from these timings or this previously observed finite case set.

## Next consumer

[#5755](https://github.com/dancxjo/conduit/issues/5755) remains the next selected consumer: compact realization from supplied committed vocative structure, reusing the accepted lexical/IPA projection and common prosody carrier. This record starts no consumer implementation and selects no learned default.

The machine-readable comparison retains every paired case, report digest and Source trace inventory. Publication and dependent runtime acceptance remain separate.

## Interpretation

Neither arm improves final attachment or vocative accuracy over the other in
this frozen comparison: both have 36/47 correct labeled attachments and 4 true
positive vocatives out of 8 (4 misses, 0 false positives). This finite zero
false-positive count is not a broad safety or precision guarantee.

Both arms commit before the last observed revision only for
`reserved-punctuation-evaluation/motivator-period`. The motivating partial
history has no commitment before its last observed revision; the reserved
initial, terminal and medial vocative controls are missed. Thus the result does
not establish a useful general prefinal vocative scorer. #6077 remains open.
All source-finality values in this diagnostic are partial; the report's
prefinal label specifically means before the last scheduled observation.

Final quality totals match, while first-commit locations differ for
`email-enronsent27_02-0013` and the reserved terminal history; the paired JSON
retains those differences. The arms use 627 versus 622 model invocations and
refuse 1810 versus 1793 illegal candidates. These counts establish no latency
win. No changes after commitment were observed within this finite profile;
that is not a general proof of an installed revision controller.

The complete result supports retaining the existing default and selecting the
supplied-committed-structure #5755 consumer. It does not promote either learned
arm, remove segmentation/capacity restrictions, or complete the broader #4907
streaming architecture.
