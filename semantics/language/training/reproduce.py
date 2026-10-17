"""Public entry: cargo xtask check parser-scorer-reproduce."""
import hashlib
import pathlib
import tempfile
import urllib.request
import train_ewt_joint_v2 as trainer

ROOT = pathlib.Path(__file__).parent
with tempfile.TemporaryDirectory(prefix='conduit-parser-reproduce-') as tmp:
    tmp = pathlib.Path(tmp)
    for name in ['train', 'dev', 'test']:
        filename = f'en_ewt-ud-{name}.conllu'
        urllib.request.urlretrieve(f'https://raw.githubusercontent.com/UniversalDependencies/UD_English-EWT/{trainer.base.PIN}/{filename}', tmp / filename)
    trainer.train_artifact(tmp, tmp / 'output')
    for path in sorted((tmp / 'output').iterdir()):
        expected = ROOT / 'ewt_joint_v2' / path.name
        if path.read_bytes() != expected.read_bytes():
            raise ValueError(f'reproduction differs: {path.name}')
        print(path.name, hashlib.sha256(path.read_bytes()).hexdigest())
