#!/usr/bin/env bash
# Fill each npm platform package with its prebuilt binary and licenses.
#
# WHY THIS IS A SCRIPT AND NOT A WORKFLOW STEP. It used to live inline in
# `release.yml`, which runs only on a tag — so its first real execution was
# always *after* the GitHub release had been published, in the job that then
# publishes to npm. npm publishes are one-way (a version cannot be republished),
# which makes this the worst possible place for first contact with a bug.
#
# It has already nearly cost a release once: GNU tar cannot read a zip, and
# `ubuntu-latest` ships GNU tar, so the Windows package would have been published
# empty. That was caught only because someone dry-ran the step by hand against
# real assets before tagging. Nothing in the repository would have caught it.
#
# As a script, `ci.yml`'s `release-dry-run` job runs THIS FILE on every pull
# request — the same code, not a copy of it. A copy would drift, and a check that
# has drifted from the thing it checks is the failure mode this repository keeps
# rediscovering.
#
# Input:  dist/ holding harness-<target>.{tar.gz,zip} for every target named in
#         npm/platforms/*/TARGET.
# Output: each npm/platforms/<pkg>/ holding its binary + both license files.
set -euo pipefail

test -d npm/platforms || { echo "::error::run from the repository root"; exit 1; }

for d in npm/platforms/*/; do
  target="$(cat "$d/TARGET")"
  case "$target" in
    *windows*)
      exe=harness.exe
      # GNU tar (which is what ubuntu-latest ships) cannot read a zip.
      python3 -m zipfile -e "dist/harness-$target.zip" dist/
      ;;
    *)
      exe=harness
      tar -xf "dist/harness-$target.tar.gz" -C dist
      ;;
  esac
  cp "dist/harness-$target/$exe" "$d/$exe"
  cp LICENSE-MIT LICENSE-APACHE "$d/"
  chmod +x "$d/$exe" || true
  test -s "$d/$exe" || { echo "::error::$d/$exe is missing or empty"; exit 1; }
  echo "  $(basename "$d") <- $target ($(stat -c%s "$d/$exe") bytes)"
done
