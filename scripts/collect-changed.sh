#!/usr/bin/env bash
# Copy the files under PATHS that differ from the commit into OUT, keeping their
# repository paths, for `.github/workflows/regenerate.yml` (D98).
#
#   bash scripts/collect-changed.sh OUT PATH...
#
# Modified and new files are copied. A deleted file cannot travel in an artifact,
# so it is listed instead, and the run fails: the generator stopped writing a file
# the repository still has, and someone has to decide whether to `git rm` it.
set -euo pipefail

cd "$(dirname "$0")/.."

out=$1
shift
mkdir -p "$out"

deleted=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  if [ -e "$f" ]; then
    cp --parents "$f" "$out/"
    echo "changed: $f"
  else
    echo "deleted: $f" >&2
    deleted=1
  fi
done < <(git ls-files --modified --others --exclude-standard -- "$@" | sort -u)

exit "$deleted"
