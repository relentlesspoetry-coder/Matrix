"""Exercise the real release preparer with disposable packages/key/repository."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import zipfile

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('release_builder',ROOT/'scripts/prepare-release.py')
builder=importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
def run(*args, cwd=None):
    return subprocess.check_output(args,cwd=cwd,stderr=subprocess.DEVNULL)

with tempfile.TemporaryDirectory(prefix='matrix-release-test-') as tmp:
    root=Path(tmp)/'repo';root.mkdir();builder.ROOT=root
    (root/'package.json').write_text('{"version":"0.6.0"}')
    (root/'src-tauri').mkdir();(root/'src-tauri/tauri.conf.json').write_text('{"version":"0.6.0"}')
    (root/'CHANGELOG.md').write_text('# Changes\n\n## 0.6.0\n\nTest release notes.\n\n## 0.5.3\n\nOlder notes.\n')
    (root/'update-core').mkdir()
    key=Path(tmp)/'key.pem';pub=Path(tmp)/'public.pem'
    run('openssl','genpkey','-algorithm','ED25519','-out',str(key))
    public=run('openssl','pkey','-in',str(key),'-pubout','-outform','DER')
    (root/'update-core/update-public-key.bin').write_bytes(public[-32:])
    run('openssl','pkey','-in',str(key),'-pubout','-out',str(pub))
    run('git','init','-q',cwd=root);run('git','add','.',cwd=root)
    run('git','-c','user.name=Release Test','-c','user.email=test@example.invalid','commit','-qm','Fixture',cwd=root)
    for kind,ext,data in [('deb','deb',b'!<arch>\nfixture'),('appimage','AppImage',b'\x7fELF0000AI\x02fixture')]:
        folder=root/f'src-tauri/target/release/bundle/{kind}';folder.mkdir(parents=True)
        (folder/f'Matrix Play_0.6.0_amd64.{ext}').write_bytes(data)
    out=Path(tmp)/'output';builder.prepare(out,key)
    manifest=json.loads((out/'update-manifest.json').read_text())
    assert manifest['notes']=='Test release notes.'
    for asset in manifest['assets'].values():
        data=(out/asset['name']).read_bytes()
        assert asset['size']==len(data)
        assert asset['sha256']==hashlib.sha256(data).hexdigest()
    run('openssl','pkeyutl','-verify','-pubin','-inkey',str(pub),'-rawin','-in',str(out/'update-manifest.json'),'-sigfile',str(out/'update-manifest.sig'))
    with zipfile.ZipFile(out/'Matrix-Play-0.6.0-source.zip') as source:
        assert 'Matrix-Play-0.6.0/package.json' in source.namelist()
        assert not any(n.endswith('.pem') or '/target/' in n for n in source.namelist())
    try:
        builder.prepare(out,key)
        raise AssertionError('Nonempty output should be rejected')
    except ValueError: pass
    wrong=Path(tmp)/'wrong.pem';run('openssl','genpkey','-algorithm','ED25519','-out',str(wrong))
    try:
        builder.prepare(Path(tmp)/'wrong-output',wrong)
        raise AssertionError('Mismatched signing key should be rejected')
    except ValueError: pass
print('PASS: real release packaging, exact version notes, signature, hashes, source archive, wrong-key and stale-output rejection (disposable fixtures)')
