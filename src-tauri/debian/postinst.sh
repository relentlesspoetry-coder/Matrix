#!/bin/sh
set -eu
# Persistent coordination inode, not a payload file replaced on each upgrade.
# The parent is root-owned and inaccessible to unprivileged writers.
install -d -m 755 -o root -g root /var/lib/matrix-play
if [ ! -e /var/lib/matrix-play/update.lock ]; then
    (umask 022; : > /var/lib/matrix-play/update.lock)
fi
