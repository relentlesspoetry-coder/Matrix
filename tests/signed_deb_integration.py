#!/usr/bin/env python3
"""Production signature/installer/APT path in a disposable user+mount+net namespace.
The dependency database is a fixture: this verifies package installation, not GUI runtime.
No host package state is mounted or writable inside the chroot.
"""
import argparse, hashlib, json, os, pathlib, re, shutil, subprocess, tempfile
p=argparse.ArgumentParser();p.add_argument('--build-only',action='store_true');p.add_argument('--seed-deb');p.add_argument('--helper');p.add_argument('--source-root');p.add_argument('--target-dir');p.add_argument('--expected',choices=['success','staging-error','hash-error','version-error']);p.add_argument('--umask',default='022',choices=['000','002','022','077']);p.add_argument('--release-dir');p.add_argument('--current',default='0.5.12');p.add_argument('--case',choices=['valid','tamper','downgrade'],default='valid');p.add_argument('--output',required=True);a=p.parse_args()
out=pathlib.Path(a.output).resolve();out.mkdir(parents=True,exist_ok=True)
if not a.helper:
    if not a.source_root: p.error('provide --source-root to build the test driver or --helper')
    source=out/'update-core'
    shutil.copytree(pathlib.Path(a.source_root)/'update-core',source,ignore=shutil.ignore_patterns('target'))
    # Copies retain mtimes; refresh them so a shared Cargo target cannot reuse a
    # fingerprint from a different fixture copy of the same package.
    for item in source.rglob('*'):
        if item.is_file(): os.utime(item,None)
    (source/'examples').mkdir(exist_ok=True)
    (source/'examples/isolation-driver.rs').write_text('fn main() {\n    assert_eq!(std::fs::read_to_string("/fixture/isolation-marker").unwrap(), "matrix isolated test\\n");\n    let current = std::env::args().nth(1).expect("fixture current version");\n    if let Err(error) = matrix_update_core::deb::install_from_stdin(&mut std::io::stdin().lock(), &current) {\n        eprintln!("{error}"); std::process::exit(1);\n    }\n}\n')
    target=pathlib.Path(a.target_dir).resolve() if a.target_dir else out/'target'
    subprocess.run(['cargo','build','--locked','--manifest-path',str(source/'Cargo.toml'),'--target-dir',str(target),'--example','isolation-driver'],check=True)
    a.helper=str(target/'debug/examples/isolation-driver')
if a.build_only:
    print(a.helper); raise SystemExit(0)
if not a.release_dir: p.error('--release-dir is required for installation tests')
root=out/'root';root.mkdir(mode=0o755)
for d in ['usr/bin','usr/sbin','bin','lib','lib64','var/cache/apt/archives/partial','var/lib/dpkg/info','var/lib/dpkg/updates','var/lib/apt/lists/partial','var/log/apt','etc/apt/sources.list.d','etc/apt/apt.conf.d','etc/apt/preferences.d','root','tmp','dev','proc','fixture']:(root/d).mkdir(parents=True,exist_ok=True)
os.chmod(root/'tmp',0o1777)
def copyexe(src,dest=None):
    src=pathlib.Path(src);dest=root/(dest or str(src).lstrip('/'));dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(src,dest);os.chmod(dest,0o755)
    linked=subprocess.run(['ldd',str(src)],text=True,capture_output=True)
    if linked.returncode and not any(word in linked.stdout+linked.stderr for word in ['statically linked','not a dynamic executable']):
        raise RuntimeError(linked.stderr)
    libs=linked.stdout
    for name in re.findall(r'(?:=>\s+)?(/[^\s()]+)',libs):
        path=root/name.lstrip('/');path.parent.mkdir(parents=True,exist_ok=True)
        if not path.exists():shutil.copyfile(name,path);os.chmod(path,0o755)
for name in ['/usr/bin/diff','/usr/sbin/start-stop-daemon','/usr/bin/apt-get','/usr/bin/dpkg-split','/usr/bin/dpkg','/usr/bin/dpkg-deb','/usr/bin/tar','/usr/bin/gzip','/usr/bin/xz','/usr/bin/sh','/usr/bin/install','/usr/bin/rm','/usr/sbin/ldconfig','/usr/lib/apt/methods/file','/usr/lib/apt/methods/store']:
    if not pathlib.Path(name).exists():continue
    copyexe(name)
copyexe('/bin/sh','bin/sh');copyexe(a.helper,'fixture/helper')
shutil.copytree('/usr/share/dpkg',root/'usr/share/dpkg',dirs_exist_ok=True)
(root/'etc/passwd').write_text('root:x:0:0:root:/root:/bin/sh\n');(root/'etc/group').write_text('root:x:0:\n');(root/'dev/null').touch()
# Installed-dependency records are synthetic; they avoid unrelated desktop runtime
# installation and any network use. The actual Matrix DEB is untouched and signed.
(root/'var/lib/dpkg/status').write_text(''.join(f'Package: {n}\nStatus: install ok installed\nPriority: optional\nSection: libs\nInstalled-Size: 0\nMaintainer: Integration Fixture\nArchitecture: amd64\nVersion: 999\nDescription: isolated dependency metadata fixture\n\n' for n in ['pkexec','libwebkit2gtk-4.1-0','libgtk-3-0']))
release=pathlib.Path(a.release_dir);manifest=(release/'update-manifest.json').read_bytes();sig=(release/'update-manifest.sig').read_bytes();data=json.loads(manifest);asset=data['assets']['linux-x86_64-deb'];deb=(release/asset['name']).read_bytes();assert hashlib.sha256(deb).hexdigest()==asset['sha256'];assert len(deb)==asset['size']
if a.case=='tamper':deb=deb[:-1]+bytes([deb[-1]^1])
wire=b'MATRIX01'+len(manifest).to_bytes(4,'big')+manifest+sig+deb
current=data['version'] if a.case=='downgrade' else a.current
(root/'fixture/request').write_bytes(wire)
(root/'fixture/isolation-marker').write_text('matrix isolated test\n')
if a.seed_deb: shutil.copyfile(a.seed_deb,root/'fixture/seed.deb')
script='set -eu; test -r "$1/fixture/isolation-marker"; mount --make-rprivate /; mount --bind /dev/null "$1/dev/null"; umask "$3"; if [ -f "$1/fixture/seed.deb" ]; then chroot "$1" /usr/bin/dpkg --install /fixture/seed.deb; fi; exec chroot "$1" /fixture/helper "$2" < "$1/fixture/request"'
cmd=['unshare','--user','--map-root-user','--mount','--net','/bin/sh','-c',script,'isolated-install',str(root),current,a.umask]
host_status=pathlib.Path('/var/lib/dpkg/status')
def host_status_hash():
    return hashlib.sha256(host_status.read_bytes()).hexdigest() if host_status.exists() else None
host_status_before=host_status_hash()
result=subprocess.run(cmd,capture_output=True,text=True)
assert host_status_hash()==host_status_before, 'Host package database changed'

(out/'stdout.log').write_text(result.stdout);(out/'stderr.log').write_text(result.stderr)
status=(root/'var/lib/dpkg/status').read_text();leftovers=list((root/'var/cache').glob('matrix-update-*'))
report={'case':a.case,'returncode':result.returncode,'installed':bool(re.search(r'Package: matrix-play\nStatus: install ok installed\n',status)),'staging_clean':not leftovers,'real_apt':True,'production_signature_key':True,'dependency_database':'synthetic installed records; no desktop-runtime claim','stdout':result.stdout,'stderr':result.stderr}
if report['installed']:
    assert re.search(r'Package: matrix-play\n(?:(?!\n\n).)*?Version: '+re.escape(data['version'])+r'\n',status,re.S), 'Wrong installed version';assert (root/'usr/bin/matrix-frontend').is_file();assert (root/'usr/lib/matrix-play/matrix-install-update').is_file();assert (root/'var/lib/matrix-play/update.lock').is_file()
(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))

expected=a.expected or {'valid':'success','tamper':'hash-error','downgrade':'version-error'}[a.case]
assert report['staging_clean'], 'Staging leaked after installer exit'
if expected=='success':
    assert result.returncode==0 and report['installed'], 'Signed DEB installation failed'
else:
    assert result.returncode!=0 and not report['installed'], 'Expected rejected request'
    message={'staging-error':'Installer requires protected root-owned staging','hash-error':'checksum','version-error':'newer'}[expected]
    assert message.lower() in result.stderr.lower(), f'Wrong failure: expected {message}'
