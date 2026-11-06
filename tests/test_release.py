"""Exercise the real release preparer with disposable packages/key/repository."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'scripts'))
from release_notes import release_notes

# Only a complete version heading identifies a release. Digits, prerelease and
# build suffixes must never turn another version's notes into a match.
for version in ['0.6.0', '0.6.1']:
    for collision in [version+'0', version+'-rc.1', version+'+build.1']:
        for changelog in [f'## {collision}\nWrong notes.\n',
                          f'Reference to ## {version}\nWrong notes.\n',
                          f'### {version}\nWrong notes.\n']:
            try:
                release_notes(changelog, version)
                raise AssertionError('A version prefix/reference is not a release heading')
            except ValueError: pass
    for changelog in [f'## {version}\n\n## Older\nOld notes.',
                      f'## {version}\nFirst notes.\n## {version}\nDuplicate notes.']:
        try:
            release_notes(changelog, version)
            raise AssertionError('Empty and duplicate version sections must be rejected')
        except ValueError: pass
assert release_notes('## 0.6.10\nWrong.\n## 0.6.1-rc.1\nWrong.\n'
                     '## 0.6.1 \t\nRight.\n### Details\nKeep details.\n'
                     '## 0.6.0\nOlder.', '0.6.1') == 'Right.\n### Details\nKeep details.'
spec=importlib.util.spec_from_file_location('release_builder',ROOT/'scripts/prepare-release.py')
builder=importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
def run(*args, cwd=None):
    return subprocess.check_output(args,cwd=cwd,stderr=subprocess.DEVNULL)

# Check Debian's actual ordering rather than assuming it matches SemVer.
run('dpkg', '--compare-versions', '0.6.1-rc.10', 'gt', '0.6.1')
run('dpkg', '--compare-versions', '0.6.1~rc.10', 'lt', '0.6.1')
run('dpkg', '--compare-versions', '0.6.2', 'gt', '0.6.1-rc.10')
run('dpkg', '--compare-versions', '0.6.1', 'gt', '0.6.0')

with tempfile.TemporaryDirectory(prefix='matrix-release-test-') as tmp:
    root=Path(tmp)/'repo';root.mkdir();builder.ROOT=root
    (root/'package.json').write_text('{"version":"0.6.0"}')
    (root/'src-tauri').mkdir();(root/'src-tauri/tauri.conf.json').write_text('{"version":"0.6.0"}')
    changelog = '# Changes\n\n## 0.6.00\nWrong prefix notes.\n\n## 0.6.0-rc.1\nWrong prerelease notes.\n\n## 0.6.0\n\nTest release notes.\n\n## 0.5.3\n\nOlder notes.\n'
    (root/'CHANGELOG.md').write_text(changelog)
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
    for index, wrong_notes in enumerate([
            '## 0.6.00\nWrong patch.\n', '## 0.6.0-rc.1\nWrong prerelease.\n',
            '## 0.6.0+build.1\nWrong build.\n', 'Reference to ## 0.6.0\nNot a heading.\n',
            '## 0.6.0\n\n## 0.5.3\nOld notes.\n',
            '## 0.6.0\nFirst.\n## 0.6.0\nDuplicate.\n']):
        (root/'CHANGELOG.md').write_text(wrong_notes)
        rejected_out = Path(tmp)/f'rejected-notes-{index}'
        try:
            builder.prepare(rejected_out, key)
            raise AssertionError('Invalid release notes must be blocked before signing')
        except ValueError as error:
            assert 'CHANGELOG.md' in str(error)
        assert not rejected_out.exists(), 'Reject before copying or signing release artifacts'
    (root/'CHANGELOG.md').write_text(changelog)
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
    for unsupported in ['0.6.1-rc.10', '0.6.1+build.1']:
        (root/'package.json').write_text(json.dumps({'version': unsupported}))
        (root/'src-tauri/tauri.conf.json').write_text(json.dumps({'version': unsupported}))
        rejected_out = Path(tmp)/f'rejected-{unsupported}'
        try:
            builder.prepare(rejected_out, key)
            raise AssertionError('The signer must reject unsupported DEB versions')
        except ValueError as error:
            assert 'DEB releases require' in str(error)
        assert not rejected_out.exists(), 'Reject before creating or signing release artifacts'
    (root/'package.json').write_text('{"version":"0.6.0"}')
    (root/'src-tauri/tauri.conf.json').write_text('{"version":"0.6.0"}')
    # Exercise the version helper against a local bare remote (no GitHub/network).
    helper_spec=importlib.util.spec_from_file_location('version_helper',ROOT/'scripts/release.py')
    helper=importlib.util.module_from_spec(helper_spec);helper_spec.loader.exec_module(helper);helper.ROOT=root
    precedence = ['0.6.0-alpha', '0.6.0-alpha.1', '0.6.0-alpha.beta',
                  '0.6.0-beta', '0.6.0-beta.2', '0.6.0-beta.11',
                  '0.6.0-rc.1', '0.6.0', '0.6.1-rc.1', '0.6.1', '0.7.0', '1.0.0']
    assert sorted(reversed(precedence), key=helper.version_key) == precedence
    assert helper.version_key('1.0.0+build.1') == helper.version_key('1.0.0+build.2')
    assert helper.version_key('0.6.1-rc.2') < helper.version_key('0.6.1-rc.10') < helper.version_key('0.6.1')
    for invalid in ['01.2.3', '1.02.3', '1.2.03', '1.2.3-01', '1.2.3-rc.01',
                    '1.2.3-', '1.2.3-rc..1', '1.2.3-rc.', '1.2.3-rc_1', '١.2.3']:
        try:
            helper.version_key(invalid)
            raise AssertionError(f'Invalid semantic version accepted: {invalid}')
        except SystemExit: pass
    (root/'.gitignore').write_text('src-tauri/target/\n')
    (root/'package-lock.json').write_text('{"version":"0.6.0","packages":{"":{"version":"0.6.0"}}}')
    (root/'src-tauri/Cargo.toml').write_text('[package]\nname = "matrix-frontend"\nversion = "0.6.0"\n')
    (root/'src-tauri/Cargo.lock').write_text('[[package]]\nname = "matrix-frontend"\nversion = "0.6.0"\n')
    (root/'web').mkdir();(root/'web/app.js').write_text('/* v0.6.0 */');(root/'web/index.html').write_text('v0.6.0')
    (root/'README.md').write_text('[v0.6.0 release](https://github.com/example/app/releases/tag/v0.6.0) Matrix-Play_0.6.0_amd64.deb\nHistorically confirmed 0.6.0.\n')
    (root/'TESTING.md').write_text('native testing release, version 0.6.0.\nHistorically confirmed 0.6.0.\n')
    (root/'CHANGELOG.md').write_text('## 0.6.1\nNew notes.\n## 0.6.1-rc.10\nSecond release candidate.\n## 0.6.1-rc.2\nFirst release candidate.\n## 0.6.0\nPrevious notes.\n')
    run('git','config','user.name','Release Test',cwd=root);run('git','config','user.email','test@example.invalid',cwd=root)
    run('git','add','.',cwd=root);run('git','commit','-qm','Version helper fixture',cwd=root)
    remote=Path(tmp)/'origin.git';run('git','init','--bare','-q',str(remote));run('git','remote','add','origin',str(remote),cwd=root)
    before = run('git','rev-parse','HEAD',cwd=root)
    for version in ['0.6.1-rc.2', '0.6.1-rc.10']:
        try:
            helper.bump(version)
            raise AssertionError('DEB prerelease must be blocked before bump/commit/push')
        except SystemExit as error:
            assert 'DEB releases require' in str(error)
        assert json.loads((root/'package.json').read_text())['version'] == '0.6.0'
        assert run('git','rev-parse','HEAD',cwd=root) == before
        assert run('git','status','--porcelain',cwd=root) == b''
        assert run('git','for-each-ref',cwd=remote) == b''
    # Exercise the actual bump gate from a clean checkout, not just its parser.
    valid_notes = (root/'CHANGELOG.md').read_text()
    for wrong_notes in ['## 0.6.10\nWrong patch.\n',
                        '## 0.6.1-rc.10\nWrong prerelease.\n',
                        '## 0.6.1+build.1\nWrong build.\n']:
        (root/'CHANGELOG.md').write_text(wrong_notes)
        run('git','add','CHANGELOG.md',cwd=root)
        run('git','commit','-qm','Invalid release note fixture',cwd=root)
        before = run('git','rev-parse','HEAD',cwd=root)
        try:
            helper.bump('0.6.1')
            raise AssertionError('A prefix collision must not authorize a release')
        except SystemExit as error:
            assert 'CHANGELOG.md' in str(error)
        assert json.loads((root/'package.json').read_text())['version'] == '0.6.0'
        assert run('git','rev-parse','HEAD',cwd=root) == before
        assert run('git','status','--porcelain',cwd=root) == b''
        assert run('git','for-each-ref',cwd=remote) == b''
    (root/'CHANGELOG.md').write_text(valid_notes)
    run('git','add','CHANGELOG.md',cwd=root)
    run('git','commit','-qm','Restore exact release notes',cwd=root)
    # A previously published raw-prerelease DEB also cannot safely promote to
    # the same core version, even though the generic SemVer comparator allows it.
    (root/'package.json').write_text('{"version":"0.6.1-rc.10"}')
    try:
        helper.bump('0.6.1')
        raise AssertionError('An APT downgrade must be blocked')
    except SystemExit as error:
        assert 'Debian package ordering' in str(error)
    (root/'package.json').write_text('{"version":"0.6.0"}')
    helper.bump('0.6.1')
    for version in ['0.6.1', '0.6.1-rc.11', '0.6.0', '0.6.2+build.1']:
        try:
            helper.bump(version)
            raise AssertionError(f'Unsupported or non-newer release accepted: {version}')
        except SystemExit: pass
        assert json.loads((root/'package.json').read_text())['version'] == '0.6.1'
    assert json.loads((root/'package.json').read_text())['version']=='0.6.1'
    assert 'Matrix-Play_0.6.1_amd64.deb' in (root/'README.md').read_text()
    assert 'Historically confirmed 0.6.0.' in (root/'README.md').read_text()
    assert 'version 0.6.1.' in (root/'TESTING.md').read_text()
    assert 'Historically confirmed 0.6.0.' in (root/'TESTING.md').read_text()
    assert b'Release Matrix Play 0.6.1' in run('git','log','main','-1','--format=%s',cwd=remote)
    print('PASS: version helper bumps, commits and pushes to a disposable local remote, retaining historical verification versions')
run('python3', str(ROOT/'scripts/release.py'), '--check-deb-version', '0.6.1')
blocked = subprocess.run(['python3', str(ROOT/'scripts/release.py'), '--check-deb-version', '0.6.1-rc.10'], capture_output=True, text=True)
assert blocked.returncode != 0 and 'DEB releases require' in blocked.stderr
print('PASS: actual dpkg ordering, unsafe DEB version rejection before build/release/signing, and generic SemVer prerelease comparison')
print('PASS: real release packaging, exact version notes, signature, hashes, source archive, wrong-key and stale-output rejection (disposable fixtures)')
