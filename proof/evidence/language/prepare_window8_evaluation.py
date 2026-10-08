#!/usr/bin/env python3
"""Freeze UD evaluation references; never invoke a model or admit a parse."""
import argparse
import collections
import hashlib
import importlib.util
import json
import sys
from pathlib import Path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def form_sequences(path):
    result = set()
    for block in path.read_text().strip().split("\n\n"):
        forms = []
        for line in block.splitlines():
            fields = line.split("\t")
            if len(fields) == 10 and fields[0].isdigit():
                forms.append(fields[1])
        if forms:
            result.add(tuple(forms))
    return result


def main(args):
    training = args.repo / "semantics/language/training"
    spec = importlib.util.spec_from_file_location("window8_reference", training / "window8_reference.py")
    reference = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(training))
    try:
        spec.loader.exec_module(reference)
        from train_ewt import semantic
    finally:
        sys.path.pop(0)
    manifest = json.loads((args.model / "manifest.json").read_text())
    profile = json.loads((args.model / "lexical_profile.json").read_text())
    assert digest(args.model / "ewt_window8.i16") == manifest["artifact_sha256"]
    assert semantic("language/parser-lexical-profile@1", (args.model / "lexical_profile.json").read_bytes()) == manifest["lexical_profile_identity"]
    assert digest(training / "window8_reference.py") == manifest["external_reference_sha256"]
    for split in ["train", "dev", "test"]:
        assert digest(args.corpus / f"en_ewt-ud-{split}.conllu") == manifest["corpus_sha256"][split]
    teaching_path = args.teaching or training / "ewt_joint_v3_window8/reviewed_teaching.json"
    assert digest(teaching_path) == manifest["reviewed_teaching_sha256"]
    teaching_identity = semantic("language/parser-teaching@1", teaching_path.read_bytes())
    if "teaching_content_identity" in manifest:
        assert teaching_identity == manifest["teaching_content_identity"]
    # All TRAIN sequences participate, including trees excluded by profile/oracle.
    train = form_sequences(args.corpus / "en_ewt-ud-train.conllu")
    train.update(tuple(row["forms"]) for row in json.loads(teaching_path.read_text()))
    authored_rows = None
    if args.authored_rows:
        authored_rows = json.loads(args.authored_rows.read_text())
        assert authored_rows and len({row["id"] for row in authored_rows}) == len(authored_rows)
        for row in authored_rows:
            count = len(row["forms"])
            assert 1 <= count <= 8
            assert all(len(row[field]) == count for field in ["pos", "heads", "relations"])
            assert row["provenance"]["model_artifact_sha256"] == manifest["artifact_sha256"]
            # Reference graph shape only; no actions enter decoder inputs.
            reference.oracle(row)
    args.output.mkdir(parents=True, exist_ok=True)
    summary = {}
    splits = ["dev", "test"] + (["authored_vocative"] if args.authored_rows else [])
    for split in splits:
        authored = split == "authored_vocative"
        if authored:
            rows, excluded = authored_rows, {}
        else:
            rows, excluded = reference.read(args.corpus / f"en_ewt-ud-{split}.conllu")
        accepted, reasons = [], collections.Counter()
        for row in rows:
            if any(form not in profile for form in row["forms"]):
                reasons["profile_oov"] += 1
                continue
            if any(pos not in profile[form] for form, pos in zip(row["forms"], row["pos"])):
                reasons["gold_pos_absent_from_train_alternatives"] += 1
                continue
            if tuple(row["forms"]) in train:
                reasons["exact_train_or_teaching_form_sequence_overlap"] += 1
                continue
            row = {key: value for key, value in row.items() if key != "oracle"}
            authored_provenance = row.get("provenance") if authored else None
            if not authored:
                row["text"] = " ".join(row["forms"])
            row["provenance"] = {
                "dataset": "separately authored evaluation" if authored else manifest["dataset"],
                "commit": None if authored else manifest["commit"],
                "split": split, "license": None if authored else manifest["license"],
                "text_policy": "original authored text" if authored else "UD forms joined with one scalar space; original whitespace not claimed",
                "labels": "reference only; never decoder constraints",
                "authored_reference": authored_provenance,
            }
            accepted.append(row)
        accepted.sort(key=lambda row: row["id"])
        path = args.output / f"{split}.json"
        path.write_text(json.dumps(accepted, indent=2) + "\n")
        summary[split] = {
            "eligible": len(rows), "profile_exclusions": excluded,
            "reference_exclusions": dict(reasons), "accepted": len(accepted),
            "sha256": digest(path), "ids": [row["id"] for row in accepted],
        }
    summary.update({
        "model_content_identity": manifest["model_content_identity"],
        "model_artifact_sha256": manifest["artifact_sha256"],
        "lexical_profile_sha256": digest(args.model / "lexical_profile.json"),
        "reference_extractor_sha256": digest(training / "window8_reference.py"),
        "teaching_sha256": digest(teaching_path),
        "teaching_content_identity": teaching_identity,
        "preparer_sha256": digest(Path(__file__)),
        "corpus_sha256": manifest["corpus_sha256"],
        "authored_rows_sha256": digest(args.authored_rows) if args.authored_rows else None,
        "exact_form_sequence_disjoint_from_all_pinned_train_and_teaching": True,
        "native_decoding_performed": False, "heldout_accuracy_claim": False,
        "scope": "reference preparation and exact sequence exclusion; full trainer provenance audit and Native replay remain required",
    })
    (args.output / "manifest.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps({split: summary[split]["accepted"] for split in splits}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ["repo", "model", "corpus", "output"]:
        parser.add_argument(f"--{argument}", type=Path, required=True)
    parser.add_argument("--teaching", type=Path,
                        help="Exact TRAIN teaching input pinned by the model manifest")
    parser.add_argument("--authored-rows", type=Path,
                        help="Separately authored evaluation references pinned to this model")
    main(parser.parse_args())
