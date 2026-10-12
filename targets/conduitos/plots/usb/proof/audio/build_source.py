#!/usr/bin/env python3
"""Assemble ordinary Source; public protocol-source-package must check it."""
import argparse
import json
from pathlib import Path


def assemble(root):
    usb = root / "targets/conduitos/plots/usb"
    def read(name):
        return (usb / name).read_text()
    audio = (root / "semantics/audio/types.conduit").read_text()
    protocol = read("protocol.conduit")
    setup = read("audio-negotiation-back.conduit")
    sections = ["with flow/zip/feedback/paired as UsbAudioSetupPair",
                audio[audio.index("type PcmSampleRepresentation"):audio.index("type AudioRenderDemand")],
                protocol[protocol.index("type UsbPcmCadence"):protocol.index("# A stale DMA completion")]]
    sections += [read(name + ".conduit") for name in (
        "control-types", "isochronous-out-types", "audio-types", "audio-block-types",
        "audio-block-profile", "audio-profile", "audio-completion-profile", "audio-playback-policy")]
    sections += [setup[setup.index("type UsbAudioSetupState"):],
                 read("audio-playback-back.conduit"), read("audio-admission-back.conduit")]
    def value(name):
        return {"type_name": name, "maximum_bytes": 4096}
    requests = [{"kind": kind, "value": value(name)} for kind, name in (
        ("singleton", "UsbAudioSetupState"), ("seeded-until", "UsbAudioSetupState"),
        ("merge", "UsbAudioPlaybackState"))]
    requests += [{"kind": "feedback-zip", "left": value("UsbAudioSetupState"), "right": value("UsbAudioSetupEvent")},
                 {"kind": "seeded-until-instance", "alias": "UsbAudioPlaybackCell", "value": value("UsbAudioPlaybackState")}]
    for kind, stem, left, right in (
        ("feedback-zip-instance", "Playback", "UsbAudioPlaybackState", "UsbIsochronousOutResult"),
        ("finite-zip-instance", "Admission", "UsbAudioAdmission", "UsbAudioAdmission"),
        ("finite-zip-instance", "RetainedPcm", "UsbAudioPcmBlock", "UsbAudioAdmission")):
        requests.append({"kind": kind, "alias": "UsbAudio" + stem + "Join",
                         "paired_alias": "UsbAudio" + stem + "Pair", "left": value(left), "right": value(right)})
    return "\n".join(sections), requests


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=Path(__file__).resolve().parents[6])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source, requests = assemble(args.workspace)
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "playback.conduit").write_text(source)
    (args.output / "specializations.json").write_text(json.dumps(requests, indent=2) + "\n")
