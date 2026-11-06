#!/usr/bin/env python3
"""Prepare launcher/compact icons from the supplied Matrix Play artwork.

The complete original PNG stays unchanged in web/assets. A code-native SVG crop
selects its central symbol, then the pinned Tauri CLI creates platform PNG sizes.
"""
import base64
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile

root=Path(__file__).resolve().parents[1]
source=root/'web/assets/matrix-play-logo.png'
raw=source.read_bytes()
if raw[:8] != b'\x89PNG\r\n\x1a\n' or struct.unpack('>II',raw[16:24]) != (1448,1086):
    raise SystemExit('This crop is for the supplied 1448x1086 logo. Update its bounds for different artwork.')
cli=root/'node_modules/.bin/tauri'
if not cli.is_file():raise SystemExit('Run npm ci in the project folder first.')
svg='<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="512" height="512" viewBox="0 0 512 512"><rect width="512" height="512" fill="#020b1c"/><svg x="0" y="52.8254" width="512" height="406.3492" viewBox="410 160 630 500" overflow="hidden"><image x="0" y="0" width="1448" height="1086" xlink:href="data:image/png;base64,'+base64.b64encode(raw).decode()+'"/></svg></svg>'
with tempfile.TemporaryDirectory(prefix='matrix-play-icons-') as tmp:
    tmp=Path(tmp)
    (tmp/'symbol.svg').write_text(svg)
    subprocess.run([str(cli),'icon',str(tmp/'symbol.svg'),'--output',str(tmp/'icons'),'--png','32','--png','128','--png','512'],check=True)
    for size in [32,128]:shutil.copy2(tmp/f'icons/{size}x{size}.png',root/f'src-tauri/icons/{size}x{size}.png')
    shutil.copy2(tmp/'icons/512x512.png',root/'src-tauri/icons/icon.png')
    shutil.copy2(tmp/'icons/512x512.png',root/'web/assets/matrix-play-icon.png')
print('Matrix Play icons prepared; original logo retained unchanged.')
