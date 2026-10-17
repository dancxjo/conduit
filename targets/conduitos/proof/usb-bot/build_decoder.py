#!/usr/bin/env python3
"""Build independent receipt checks in a new, explicitly owned work directory."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
p=argparse.ArgumentParser()
p.add_argument('--workspace',required=True)
p.add_argument('--output',required=True)
a=p.parse_args()
workspace=Path(a.workspace).resolve()
output=Path(a.output).resolve()
assert (workspace/'architecture/core/Cargo.toml').is_file()
output.mkdir(exist_ok=False)
(output/'src').mkdir()
source=Path(__file__).resolve().parent
shutil.copy2(source/'decoder_main.rs',output/'src/main.rs')
shutil.copy2(source/'verify_storage.rs',output/'src/verify_storage.rs')
(output/'Cargo.toml').write_text('[package]\nname="usb-bot-receipt-decoder"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[dependencies]\nconduit-core={path='+json.dumps(str(workspace/'architecture/core'))+'}\n')
subprocess.run(['cargo','+stable','build','--offline','--manifest-path',str(output/'Cargo.toml')],check=True)
print(output/'target/debug/usb-bot-receipt-decoder')
