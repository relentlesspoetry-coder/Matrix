#!/bin/bash
# Run only CI fixtures under the isolated account. No credentials, runner service
# environment, Rust caches, or production-user data are shared with that account.
set -euo pipefail
if [ "$#" -eq 0 ]; then
  echo 'Usage: run-ci-as-test-user.sh COMMAND [ARG ...]' >&2
  exit 2
fi
uid=$(id -u matrix-test)
if [ "$uid" = 0 ] || [ "$uid" = "$(id -u)" ]; then
  echo 'The process-test account must be non-root and separate from the runner.' >&2
  exit 1
fi
settings=(
  HOME=/home/matrix-test USER=matrix-test LOGNAME=matrix-test
  PATH=/home/matrix-test/bin:/usr/bin:/bin
  XDG_RUNTIME_DIR=/home/matrix-test/runtime RUST_BACKTRACE=1
)
# Deliberately narrow; never sudo --preserve-env or copy GitHub/token variables.
for name in APPIMAGE_EXTRACT_AND_RUN WEBKIT_DISABLE_COMPOSITING_MODE WEBKIT_DISABLE_DMABUF_RENDERER LIBGL_ALWAYS_SOFTWARE; do
  if [[ -v "$name" ]]; then settings+=("$name=${!name}"); fi
done
exec sudo --user=matrix-test --group=matrix-test -- env -i "${settings[@]}" /bin/sh -c 'cd /home/matrix-test/work; exec "$@"' sh "$@"
