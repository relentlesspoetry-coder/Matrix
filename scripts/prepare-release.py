#!/usr/bin/env python3
"""Prepare immutable Linux release assets and an Ed25519 signed update manifest."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
from release_notes import release_notes

ROOT = Path(__file__).resolve().parents[1]

def prepare(out, key):
    version = json.loads((ROOT/'package.json').read_text())['version']
    native = json.loads((ROOT/'src-tauri/tauri.conf.json').read_text())['version']
    if native != version:
        raise ValueError('Application versions disagree')
    # This signer always publishes a DEB. Tauri 2.12.4 has no DEB version
    # override and writes the raw application version to its control file.
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise ValueError('DEB releases require a numeric X.Y.Z version; use GitHub testing/prerelease status instead of a version suffix')
    notes = release_notes((ROOT/'CHANGELOG.md').read_text(), version)
    out.mkdir(parents=True, exist_ok=True)
    if any(out.iterdir()):
        raise ValueError('Use an empty release output folder')
    assets = {}
    for kind, extension in [('deb','deb'),('appimage','AppImage')]:
        source = ROOT/f'src-tauri/target/release/bundle/{kind}/Matrix Play_{version}_amd64.{extension}'
        name = f'Matrix-Play_{version}_amd64.{extension}'
        target = out/name
        shutil.copyfile(source, target)
        assets[f'linux-x86_64-{kind}'] = dict(name=name, size=target.stat().st_size, sha256=hashlib.sha256(target.read_bytes()).hexdigest())
    manifest = dict(schema=1, repository='relentlesspoetry-coder/Matrix', version=version, tag='v'+version, notes=notes, assets=assets)
    path = out/'update-manifest.json'
    path.write_text(json.dumps(manifest,sort_keys=True,indent=2)+'\n')
    public = subprocess.check_output(['openssl','pkey','-in',str(key),'-pubout','-outform','DER'],stderr=subprocess.DEVNULL)
    if public[-32:] != (ROOT/'update-core/update-public-key.bin').read_bytes():
        raise ValueError('Release signing key does not match the application verification key')
    subprocess.run(['openssl','pkeyutl','-sign','-inkey',str(key),'-rawin','-in',str(path),'-out',str(out/'update-manifest.sig')],check=True)
    if (out/'update-manifest.sig').stat().st_size != 64:
        raise ValueError('Unexpected signature length')
    with (out/f'Matrix-Play-{version}-source.zip').open('wb') as stream:
        subprocess.run(['git','archive','--format=zip',f'--prefix=Matrix-Play-{version}/','HEAD'],cwd=ROOT,stdout=stream,check=True)
    (out/'release-notes.md').write_text(notes+'\n\nLinux x86-64 testing release. See README.md and TESTING.md for setup and verified limits.\n')
    sums = ''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in sorted(out.iterdir()) if p.name not in ['SHA256SUMS','release-notes.md'])
    (out/'SHA256SUMS').write_text(sums)
    print(f'Prepared signed Matrix Play {version} release')

if __name__ == '__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--out',type=Path,default=ROOT/'release-out')
    parser.add_argument('--key-file',type=Path)
    args=parser.parse_args()
    if args.key_file:
        prepare(args.out.resolve(),args.key_file.resolve())
    else:
        secret=os.environ.get('MATRIX_UPDATE_SIGNING_KEY')
        if not secret:
            raise SystemExit('The release signing secret is missing')
        with tempfile.TemporaryDirectory(prefix='matrix-release-key-') as tmp:
            key=Path(tmp)/'key.pem'
            fd=os.open(key,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
            with os.fdopen(fd,'w') as stream:
                stream.write(secret)
            prepare(args.out.resolve(),key)
