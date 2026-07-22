#!/usr/bin/env bash
# CI guard: every published blog post must render a hero image.
#
# Why: `heroImage` is optional in the content schema, so a post that ships WITHOUT a
# heroImage frontmatter line (or with one whose asset doesn't resolve) still builds and
# renders — just with no hero. So a green build does NOT prove a hero is present. This
# checks the BUILT HTML, not the frontmatter, catching both the missing-line and the
# unresolved-asset cases. The hero renders via `<Image width={1020} height={510}>` in
# BlogPost.astro, so a post page with a hero contains that 1020x510 <img>.
#
# Run (from blog/): npm run build && npm run check:heroes
set -euo pipefail
cd "$(dirname "$0")/.."   # -> blog/
DIST="dist/blog"
[ -d "$DIST" ] || { echo "check:heroes: no $DIST — run 'npm run build' first" >&2; exit 1; }

fail=0
for post in src/content/blog/*.md src/content/blog/*.mdx; do
  [ -e "$post" ] || continue
  slug="$(basename "$post")"; slug="${slug%.*}"
  html="$DIST/$slug/index.html"
  if [ ! -f "$html" ]; then
    echo "check:heroes: FAIL $slug — no built page at $html" >&2; fail=1; continue
  fi
  # Hero = <Image width={1020} height={510}> -> width="1020" + height="510" in the <img>.
  if grep -q 'width="1020"' "$html" && grep -q 'height="510"' "$html"; then
    :
  else
    echo "check:heroes: FAIL $slug — no hero rendered. Add a heroImage frontmatter line pointing at a real asset in src/assets/ (see any other post)." >&2
    fail=1
  fi
done

[ "$fail" -eq 0 ] && echo "check:heroes: OK — every post renders a hero ($(ls "$DIST" | wc -l) pages)"
exit "$fail"
