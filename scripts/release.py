#!/usr/bin/env python3
"""Bump a clean, reviewed main checkout, commit, and push. CI tests/signs/publishes."""
import argparse
import json
from pathlib import Path
import re
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def git(*args):
    return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()

def bump(version):
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?',version):
        raise SystemExit('Use a semantic version such as 0.5.3')
    old=json.loads((ROOT/'package.json').read_text())['version']
    if tuple(map(int,version.split('-')[0].split('.'))) <= tuple(map(int,old.split('-')[0].split('.'))):
        raise SystemExit('Choose a version newer than the current version')
    if git('branch','--show-current')!='main' or git('status','--porcelain'):
        raise SystemExit('Commit and review all changes on main before releasing')
    if f'## {version}' not in (ROOT/'CHANGELOG.md').read_text():
        raise SystemExit('Commit release notes in CHANGELOG.md first')
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
    for name in ['web/app.js','web/index.html','README.md','TESTING.md']:
        p=ROOT/name;p.write_text(p.read_text().replace(old,version))
    subprocess.run(['git','add','package.json','package-lock.json','src-tauri/tauri.conf.json','src-tauri/Cargo.toml','src-tauri/Cargo.lock','web/app.js','web/index.html','README.md','TESTING.md'],cwd=ROOT,check=True)
    subprocess.run(['git','commit','-m',f'Release Matrix Play {version}'],cwd=ROOT,check=True)
    # Persist the user's instruction: every new version is pushed, never left local.
    subprocess.run(['git','-c','credential.helper=!gh auth git-credential','push','origin','main'],cwd=ROOT,check=True)
    print(f'Pushed {version}. GitHub Actions will test, sign and publish it automatically.')

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('version');args=parser.parse_args();bump(args.version)
