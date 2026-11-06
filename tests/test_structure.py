from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory

root = Path(__file__).resolve().parents[1]
expected = [
 ('Switch','Switch Games'), ('PS1','PS1 Games'),('PS2','PS2 Games'),('PS3','PS3 Games'),
 ('PSP','PSP Games'), ('N64','N64 Games'),('GameCube','GameCube Games'),
 ('Dreamcast','Dreamcast Games'),('Wii','Wii Games'),('Wii U','Wii U Games'),
 ('Game Boy','Game Boy Games'),('GBA','GBA Games'),('Nintendo DS','Nintendo DS Games'),
 ('Nintendo 3DS','Nintendo 3DS Games'),
 ('PS4','PS4 Games'), ('PS5','PS5 Games'), ('PS Vita','PS Vita Games'), ('Switch 2','Switch 2 Games'),
]
with TemporaryDirectory() as temp:
    dest = Path(temp)/'Matrix'
    script = root / 'scripts' / 'setup-folders.sh'
    subprocess.run([str(script),str(dest)],check=True,capture_output=True)
    assert len([x for x in dest.iterdir() if x.is_dir()]) == 18
    assert all((dest/console/games).is_dir() for console,games in expected)
    (dest/'PS2'/'PS2 Games'/'keep.iso').write_text('original fixture')
    subprocess.run([str(script),str(dest)],check=True,capture_output=True)
    assert (dest/'PS2'/'PS2 Games'/'keep.iso').read_text() == 'original fixture'
print('PASS: 18 managed console folders, exact naming, idempotent folder creation preserves existing files')
