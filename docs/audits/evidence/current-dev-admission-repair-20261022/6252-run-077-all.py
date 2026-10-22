import subprocess,sys
from pathlib import Path
base=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit')
subprocess.run([sys.executable,str(base/'work/6252-run-077-focused.py')],cwd=base,check=True)
with (base/'work/6252-077a2a535-sdk-errors.log').open('w') as log:
 subprocess.run(['node','--test','targets/browser/sdk/browser-sdk-errors.test.mjs'],cwd=base/'work/6252-native-fixtures',stdout=log,stderr=subprocess.STDOUT,check=True)
subprocess.run([sys.executable,str(base/'work/6252-run-077-catalog-and-strict.py')],cwd=base,check=True)
