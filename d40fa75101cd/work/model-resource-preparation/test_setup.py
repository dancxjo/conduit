from pathlib import Path
import json
proj=Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907');r=json.loads((proj/'outputs/fargan-model-compute-session/session-receipt.json').read_text())['basis'];p=Path('work/model-resource-preparation/original-basis.json');p.write_text(json.dumps({k:r[k]for k in ['model_content_identity','model_descriptor_identity','model_raw_sha256','model_descriptor','resource_binding']},indent=2)+'\n')
