#!/usr/bin/env bash
# Print PATH with /usr/bin pinned first when the real docker binary lives
# there, so a wrapper shim earlier on PATH cannot shadow it during a deploy.
set -euo pipefail
if [ -x /usr/bin/docker ]; then
    printf '%s\n' "/usr/bin:$PATH"
else
    printf '%s\n' "$PATH"
fi
