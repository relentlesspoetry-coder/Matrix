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
    run('git','init','-q','-b','main',cwd=root);run('git','add','.',cwd=root)
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
    # Exercise the version helper against a local bare remote (no GitHub/network).
    helper_spec=importlib.util.spec_from_file_location('version_helper',ROOT/'scripts/release.py')
    helper=importlib.util.module_from_spec(helper_spec);helper_spec.loader.exec_module(helper);helper.ROOT=root
    (root/'.gitignore').write_text('src-tauri/target/\n')
    (root/'package-lock.json').write_text('{"version":"0.6.0","packages":{"":{"version":"0.6.0"}}}')
    (root/'src-tauri/Cargo.toml').write_text('[package]\nname = "matrix-frontend"\nversion = "0.6.0"\n')
    (root/'src-tauri/Cargo.lock').write_text('[[package]]\nname = "matrix-frontend"\nversion = "0.6.0"\n')
    (root/'web').mkdir();(root/'web/app.js').write_text('/* v0.6.0 */');(root/'web/index.html').write_text('v0.6.0')
    (root/'README.md').write_text('[v0.6.0 release](https://github.com/example/app/releases/tag/v0.6.0) Matrix-Play_0.6.0_amd64.deb\nHistorically confirmed 0.6.0.\n')
    (root/'TESTING.md').write_text('native testing release, version 0.6.0.\nHistorically confirmed 0.6.0.\n')
    (root/'CHANGELOG.md').write_text('## 0.6.1\nNew notes.\n## 0.6.0\nPrevious notes.\n')
    run('git','config','user.name','Release Test',cwd=root);run('git','config','user.email','test@example.invalid',cwd=root)
    run('git','add','.',cwd=root);run('git','commit','-qm','Version helper fixture',cwd=root)
    remote=Path(tmp)/'origin.git';run('git','init','--bare','-q',str(remote));run('git','remote','add','origin',str(remote),cwd=root)
    helper.bump('0.6.1')
    assert json.loads((root/'package.json').read_text())['version']=='0.6.1'
    assert 'Matrix-Play_0.6.1_amd64.deb' in (root/'README.md').read_text()
    assert 'Historically confirmed 0.6.0.' in (root/'README.md').read_text()
    assert 'version 0.6.1.' in (root/'TESTING.md').read_text()
    assert 'Historically confirmed 0.6.0.' in (root/'TESTING.md').read_text()
    assert b'Release Matrix Play 0.6.1' in run('git','log','main','-1','--format=%s',cwd=remote)
    print('PASS: version helper bumps, commits and pushes to a disposable local remote, retaining historical verification versions')
print('PASS: real release packaging, exact version notes, signature, hashes, source archive, wrong-key and stale-output rejection (disposable fixtures)')
