#!/usr/bin/env bash
# Optional standalone folder initialization; native Matrix Play performs this on first launch.
set -euo pipefail
if [[ -n "${1:-}" ]]; then
  root="$1"
elif [[ -d "${HOME:?HOME is not set}/Games/Matrix" ]]; then
  root="$HOME/Games/Matrix"
else
  root="$HOME/Games/Matrix Play"
fi
while IFS='|' read -r console library; do
  mkdir -p -- "$root/$console/$library"
done <<'LIBRARIES'
Switch|Switch Games
PS1|PS1 Games
PS2|PS2 Games
PS3|PS3 Games
PSP|PSP Games
N64|N64 Games
GameCube|GameCube Games
Dreamcast|Dreamcast Games
Wii|Wii Games
Wii U|Wii U Games
Game Boy|Game Boy Games
GBA|GBA Games
Nintendo DS|Nintendo DS Games
Nintendo 3DS|Nintendo 3DS Games
PS4|PS4 Games
PS5|PS5 Games
PS Vita|PS Vita Games
Switch 2|Switch 2 Games
Sega Genesis|Sega Genesis Games
Sega Saturn|Sega Saturn Games
Super Nintendo|Super Nintendo Games
Nintendo NES|Nintendo NES Games
Xbox|Xbox Games
Xbox 360|Xbox 360 Games
LIBRARIES
printf 'Matrix Play folders ready: %s\n' "$root"
