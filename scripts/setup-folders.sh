#!/usr/bin/env bash
# Optional standalone folder initialization; native Matrix performs this on first launch.
set -euo pipefail
root="${1:-${HOME:?HOME is not set}/Games/Matrix}"
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
LIBRARIES
printf 'Matrix folders ready: %s\n' "$root"
