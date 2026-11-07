"""Inspect a DEB without installing it or executing any packaged program.

Run without arguments for synthetic archive self-tests, or pass one or more built
.deb paths to verify their identity and privileged update helper packaging.
"""
import argparse
import io
import json
from pathlib import Path
import re
import struct
import subprocess
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
HELPER = 'usr/lib/matrix-play/matrix-install-update'
FRONTEND = 'usr/bin/matrix-frontend'
COORDINATOR = 'matrix-update-coordinator'
PERSISTENT_GUARD = 'var/lib/matrix-play/update.lock'
POSTINST = ROOT / 'src-tauri/debian/postinst.sh'
PACKAGE = 'matrix-play'
AMD64_ELF = (2, 1, 62)  # ELF64, little-endian, EM_X86_64


def elf_architecture(header, path):
    if len(header) < 64 or header[:4] != b'\x7fELF':
        raise ValueError(f'{path}: missing or truncated ELF header')
    if header[4] not in (1, 2) or header[5] not in (1, 2) or header[6] != 1:
        raise ValueError(f'{path}: invalid ELF identification')
    machine = struct.unpack('<H' if header[5] == 1 else '>H', header[18:20])[0]
    return header[4], header[5], machine


def verify_deb(package, version):
    for field, expected in [('Package', PACKAGE), ('Version', version), ('Architecture', 'amd64')]:
        actual = subprocess.check_output(
            ['dpkg-deb', '--field', str(package), field], text=True
        ).strip()
        if actual != expected:
            raise ValueError(f'{field}: expected {expected!r}, got {actual!r}')
    dependencies = subprocess.check_output(
        ['dpkg-deb', '--field', str(package), 'Depends'], text=True
    ).strip()
    # A recommendation, name substring or optional alternative does not ensure
    # /usr/bin/pkexec is installed for the secure in-app system upgrade path.
    if not any(re.fullmatch(r'pkexec(?::(?:any|native|amd64))?(?:\s*\([^)]+\))?',
                            dependency.strip())
               for dependency in dependencies.split(',')):
        raise ValueError('Depends: the secure update installer requires pkexec')
    # Inspect the control archive too: the stable installer guard must be
    # provisioned by a safe maintainer script, never by installing a payload
    # file that would replace its locked inode. Nothing here is executed.
    with tempfile.TemporaryFile() as control:
        subprocess.run(['dpkg-deb', '--ctrl-tarfile', str(package)], stdout=control, check=True)
        control.seek(0)
        with tarfile.open(fileobj=control, mode='r:') as archive:
            scripts = [member for member in archive if member.name.removeprefix('./').lstrip('/') == 'postinst']
            if len(scripts) != 1:
                raise ValueError('postinst: missing or duplicate control archive member')
            script = scripts[0]
            if not script.isfile():
                raise ValueError('postinst: must be a regular file')
            if (script.uid, script.gid) != (0, 0):
                raise ValueError('postinst: must be owned by root:root')
            if script.mode & 0o022 or script.mode & 0o111 != 0o111:
                raise ValueError('postinst: must be executable and not group/other-writable')
            if script.mode & 0o7000:
                raise ValueError('postinst: unexpected special permission bits')
            if script.size > 65536:
                raise ValueError('postinst: unexpectedly large script')
            with archive.extractfile(script) as source:
                body = source.read(65537)
            # Tauri may surround the repository hook with its own boilerplate;
            # the exact reviewed hook must remain present and unmodified.
            if not body.startswith(b'#!') or POSTINST.read_bytes().strip() not in body:
                raise ValueError('postinst: missing expected persistent-lock provisioning hook')
    # Use a temporary file rather than buffering an entire installer in memory.
    # Nothing is extracted; tar metadata and just the ELF headers are read.
    with tempfile.TemporaryFile() as payload:
        subprocess.run(['dpkg-deb', '--fsys-tarfile', str(package)], stdout=payload, check=True)
        payload.seek(0)
        with tarfile.open(fileobj=payload, mode='r:') as archive:
            found = {}
            for member in archive:
                name = member.name
                while name.startswith('./'):
                    name = name[2:]
                name = name.lstrip('/')
                if name == PERSISTENT_GUARD:
                    raise ValueError('Persistent update guard must not be a replaceable package payload')
                if name.startswith('usr/lib/') and name.endswith('/' + COORDINATOR):
                    name = COORDINATOR
                if name not in (HELPER, FRONTEND, COORDINATOR):
                    continue
                if name in found:
                    raise ValueError(f'{name}: duplicate archive member')
                if not member.isfile():
                    raise ValueError(f'{name}: must be a regular file')
                if name in (HELPER, COORDINATOR):
                    if (member.uid, member.gid) != (0, 0):
                        raise ValueError(f'{name}: must be owned by root:root')
                    if member.mode & 0o022 or member.mode & 0o111 != 0o111:
                        raise ValueError(f'{name}: must be executable and not group/other-writable')
                    if member.mode & 0o7000:
                        raise ValueError(f'{name}: unexpected special permission bits')
                with archive.extractfile(member) as binary:
                    found[name] = elf_architecture(binary.read(64), name)
            for name in (HELPER, FRONTEND, COORDINATOR):
                if name not in found:
                    raise ValueError(f'{name}: missing from package')
            if found[FRONTEND] != AMD64_ELF:
                raise ValueError(f'{FRONTEND}: expected amd64 ELF architecture')
            for name in (HELPER, COORDINATOR):
                if found[name] != found[FRONTEND]:
                    raise ValueError(f'{name}: ELF architecture does not match frontend')


def synthetic_elf(machine=62, elf_class=2):
    header = bytearray(64)
    header[:7] = b'\x7fELF' + bytes((elf_class, 1, 1))
    struct.pack_into('<H', header, 18, machine)
    return bytes(header)


def fixture_deb(path, *, package=PACKAGE, version='1.2.3', architecture='amd64',
                helper=True, uid=0, gid=0, mode=0o755, kind=tarfile.REGTYPE,
                helper_elf=None, frontend_elf=None, duplicate=False, depends='pkexec', coordinator=True,
                postinst=True, postinst_uid=0, postinst_gid=0, postinst_mode=0o755,
                postinst_kind=tarfile.REGTYPE, postinst_body=None, postinst_duplicate=False, persistent_payload=False):
    """Write a minimal ar/DEB with controlled tar metadata, without root access."""
    def tar_bytes(entries):
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode='w') as archive:
            for entry, data in entries:
                entry.size = len(data) if entry.isfile() else 0
                archive.addfile(entry, io.BytesIO(data) if entry.isfile() else None)
        return output.getvalue()

    control = f'Package: {package}\nVersion: {version}\nArchitecture: {architecture}\nDepends: {depends}\nMaintainer: Fixture <fixture@example.invalid>\nDescription: Archive test only\n'.encode()
    entries = [(tarfile.TarInfo('./' + FRONTEND), frontend_elf if frontend_elf is not None else synthetic_elf())]
    if helper:
        entry = tarfile.TarInfo('./' + HELPER)
        entry.uid, entry.gid, entry.mode, entry.type = uid, gid, mode, kind
        entry.linkname = '/bin/true' if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE) else ''
        entries.append((entry, helper_elf if helper_elf is not None else synthetic_elf()))
        if duplicate:
            entries.append(entries[-1])
    if coordinator:
        entry = tarfile.TarInfo('./usr/lib/matrix-play/' + COORDINATOR)
        entry.mode = 0o755
        entries.append((entry, synthetic_elf()))
    controls = [(tarfile.TarInfo('./control'), control)]
    if postinst:
        entry = tarfile.TarInfo('./postinst')
        entry.uid, entry.gid, entry.mode, entry.type = postinst_uid, postinst_gid, postinst_mode, postinst_kind
        entry.linkname = 'control' if postinst_kind in (tarfile.SYMTYPE, tarfile.LNKTYPE) else ''
        controls.append((entry, POSTINST.read_bytes() if postinst_body is None else postinst_body))
        if postinst_duplicate:
            controls.append(controls[-1])
    if persistent_payload:
        entries.append((tarfile.TarInfo('./' + PERSISTENT_GUARD), b''))
    with path.open('wb') as output:
        output.write(b'!<arch>\n')
        for name, data in [('debian-binary', b'2.0\n'),
                           ('control.tar', tar_bytes(controls)),
                           ('data.tar', tar_bytes(entries))]:
            output.write(f'{name + "/":<16}{0:<12}{0:<6}{0:<6}{"100644":<8}{len(data):<10}`\n'.encode())
            output.write(data)
            if len(data) % 2:
                output.write(b'\n')


class PackageTests(unittest.TestCase):
    def test_valid_package(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'valid.deb'
            for depends in ['pkexec', 'libgtk-3-0, pkexec (>= 122), libwebkit2gtk-4.1-0',
                            'pkexec:amd64']:
                with self.subTest(depends=depends):
                    fixture_deb(path, depends=depends)
                    verify_deb(path, '1.2.3')

    def test_bundle_declares_installer_dependency(self):
        config = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())
        self.assertIn('pkexec', config['bundle']['linux']['deb']['depends'])
        self.assertEqual(config['bundle']['resources']['target/release/matrix-update-coordinator'], 'matrix-update-coordinator')
        hook = ROOT / 'src-tauri' / config['bundle']['linux']['deb']['postInstallScript']
        self.assertIn('update.lock', hook.read_text())
        self.assertIn('--bin matrix-update-coordinator', config['build']['beforeBuildCommand'])

    def test_postinst_and_persistent_guard_are_validated_without_execution(self):
        cases = [
            ({'postinst': False}, 'missing'),
            ({'postinst_duplicate': True}, 'duplicate'),
            ({'postinst_uid': 1000}, 'root:root'),
            ({'postinst_gid': 1000}, 'root:root'),
            ({'postinst_mode': 0o644}, 'executable'),
            ({'postinst_mode': 0o775}, 'group/other-writable'),
            ({'postinst_mode': 0o4755}, 'special permission'),
            ({'postinst_kind': tarfile.SYMTYPE}, 'regular file'),
            ({'postinst_kind': tarfile.LNKTYPE}, 'regular file'),
            ({'postinst_body': b'#!/bin/sh\nexit 0\n'}, 'provisioning hook'),
            ({'postinst_body': b'x' * 65537}, 'large script'),
            ({'persistent_payload': True}, 'replaceable package payload'),
        ]
        with tempfile.TemporaryDirectory() as directory:
            for index, (options, expected) in enumerate(cases):
                with self.subTest(options=options.keys()):
                    path = Path(directory) / f'unsafe-hook-{index}.deb'
                    fixture_deb(path, **options)
                    with self.assertRaisesRegex(ValueError, expected):
                        verify_deb(path, '1.2.3')

    def test_invalid_packages(self):
        cases = [
            ({'depends': ''}, 'requires pkexec'),
            ({'depends': 'libgtk-3-0, libwebkit2gtk-4.1-0'}, 'requires pkexec'),
            ({'depends': 'not-pkexec'}, 'requires pkexec'),
            ({'depends': 'pkexec | other-package'}, 'requires pkexec'),
            ({'helper': False}, 'missing'),
            ({'coordinator': False}, 'missing'),
            ({'uid': 1000}, 'root:root'),
            ({'gid': 1000}, 'root:root'),
            ({'mode': 0o775}, 'group/other-writable'),
            ({'mode': 0o757}, 'group/other-writable'),
            ({'mode': 0o644}, 'executable'),
            ({'mode': 0o4755}, 'special permission'),
            ({'kind': tarfile.SYMTYPE}, 'regular file'),
            ({'kind': tarfile.LNKTYPE}, 'regular file'),
            ({'duplicate': True}, 'duplicate'),
            ({'helper_elf': synthetic_elf(machine=183)}, 'does not match'),
            ({'helper_elf': synthetic_elf(elf_class=1)}, 'does not match'),
            ({'frontend_elf': synthetic_elf(machine=183)}, 'expected amd64'),
            ({'helper_elf': b'#!/bin/sh\n'}, 'ELF header'),
            ({'package': 'other'}, 'Package:'),
            ({'version': '1.2.4'}, 'Version:'),
            ({'architecture': 'arm64'}, 'Architecture:'),
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'invalid.deb'
            for changes, error in cases:
                with self.subTest(**changes):
                    fixture_deb(path, **changes)
                    with self.assertRaisesRegex(ValueError, error):
                        verify_deb(path, '1.2.3')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('packages', nargs='*', type=Path)
    args = parser.parse_args()
    if args.packages:
        version = json.loads((ROOT / 'package.json').read_text())['version']
        for package in args.packages:
            verify_deb(package, version)
            print(f'Validated {package.name}: {PACKAGE} {version} amd64, pkexec dependency and root-owned ELF update helper')
    else:
        unittest.main(argv=[__file__])
