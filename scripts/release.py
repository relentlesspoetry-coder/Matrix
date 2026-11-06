#!/usr/bin/env python3
"""Bump a clean, reviewed main checkout, commit, and push. CI tests/signs/publishes."""
import argparse
import json
from pathlib import Path
import re
import subprocess
from release_notes import release_notes

ROOT=Path(__file__).resolve().parents[1]
def git(*args):
    return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()

def version_key(version):
    """SemVer precedence, with build metadata ignored and stable above prerelease."""
    match = re.fullmatch(
        r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)'
        r'(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?'
        r'(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?', version)
    if not match:
        raise SystemExit('Use a semantic version such as 0.5.3 or 0.6.0-rc.1')
    major, minor, patch, prerelease, _ = match.groups()
    identifiers = []
    for identifier in prerelease.split('.') if prerelease is not None else []:
        if identifier.isascii() and identifier.isdigit():
            if len(identifier) > 1 and identifier.startswith('0'):
                raise SystemExit('Numeric prerelease identifiers cannot have leading zeroes')
            identifiers.append((0, int(identifier)))
        else:
            identifiers.append((1, identifier))
    return (int(major), int(minor), int(patch), prerelease is None, tuple(identifiers))

def require_deb_version(version):
    """This release pipeline always includes a DEB with Tauri's raw version."""
    key = version_key(version)
    if not key[3] or '+' in version:
        raise SystemExit('DEB releases require a numeric X.Y.Z version: Tauri does not map '
                         'SemVer prereleases to Debian ordering. Mark a numeric release '
                         'as prerelease/testing on GitHub instead.')
    return key

def bump(version):
    candidate = require_deb_version(version)
    old=json.loads((ROOT/'package.json').read_text())['version']
    if candidate <= version_key(old):
        raise SystemExit('Choose a version newer than the current version')
    # Also protect installations of an older, raw-prerelease DEB: moving from
    # 0.6.1-rc.10 to 0.6.1 would be an upgrade in SemVer but a downgrade in APT.
    if subprocess.run(['dpkg','--compare-versions',version,'gt',old], check=False).returncode:
        raise SystemExit('Choose a version newer under Debian package ordering too')
    if git('branch','--show-current')!='main' or git('status','--porcelain'):
        raise SystemExit('Commit and review all changes on main before releasing')
    try:
        release_notes((ROOT/'CHANGELOG.md').read_text(), version)
    except ValueError as error:
        raise SystemExit(str(error)) from error
    for name in ['package.json','package-lock.json','src-tauri/tauri.conf.json']:
        p=ROOT/name;data=json.loads(p.read_text());data['version']=version
        if name=='package-lock.json': data['packages']['']['version']=version
        p.write_text(json.dumps(data,indent=2)+'\n')
    for name in ['src-tauri/Cargo.toml','src-tauri/Cargo.lock']:
        p=ROOT/name;s=p.read_text()
        if name.endswith('toml'):
            s=s.replace(f'version = "{old}"',f'version = "{version}"',1)
        else:
            s=s.replace(f'name = "matrix-frontend"\nversion = "{old}"',f'name = "matrix-frontend"\nversion = "{version}"',1)
        p.write_text(s)
    for name in ['web/app.js','web/index.html']:
        p=ROOT/name;p.write_text(p.read_text().replace(old,version))
    for name in ['README.md','TESTING.md']:
        p=ROOT/name;s=p.read_text()
        # Update current install instructions without rewriting dated evidence.
        for before,after in [
            (f'Matrix-Play_{old}_amd64',f'Matrix-Play_{version}_amd64'),
            (f'/releases/tag/v{old}',f'/releases/tag/v{version}'),
            (f'[v{old} release]',f'[v{version} release]'),
            (f'[v{old} GitHub release]',f'[v{version} GitHub release]'),
            (f'native testing release, version {old}.',f'native testing release, version {version}.'),
            (f'install the v{old} DEB',f'install the v{version} DEB'),
        ]: s=s.replace(before,after)
        p.write_text(s)
    subprocess.run(['git','add','package.json','package-lock.json','src-tauri/tauri.conf.json','src-tauri/Cargo.toml','src-tauri/Cargo.lock','web/app.js','web/index.html','README.md','TESTING.md'],cwd=ROOT,check=True)
    subprocess.run(['git','commit','-m',f'Release Matrix Play {version}'],cwd=ROOT,check=True)
    # Persist the user's instruction: every new version is pushed, never left local.
    subprocess.run(['git','-c','credential.helper=!gh auth git-credential','push','origin','main'],cwd=ROOT,check=True)
    print(f'Pushed {version}. GitHub Actions will test, sign and publish it automatically.')

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('version', nargs='?')
    parser.add_argument('--check-deb-version', action='store_true', help='Validate the current or supplied version without modifying files')
    args=parser.parse_args()
    if args.check_deb_version:
        require_deb_version(args.version or json.loads((ROOT/'package.json').read_text())['version'])
    elif args.version:
        bump(args.version)
    else:
        parser.error('a release version is required')
