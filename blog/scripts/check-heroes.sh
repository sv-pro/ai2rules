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
  # Drafts are deliberately not built (`draft: true`), so requiring a page for
  # them would fail this guard on exactly the posts that are not finished yet.
  # Matched on the frontmatter block only, so the word cannot leak in from prose.
  if awk '/^---$/{n++; next} n==1' "$post" | grep -qE '^draft:[[:space:]]*true[[:space:]]*$'; then
    echo "check:heroes: skip $slug (draft)"
    continue
  fi
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

# --- Second guard: dedicated art must not sit unused -------------------------
#
# The check above asserts every post renders *a* hero. That was true for months
# while three posts rendered a generic `blog-placeholder-*` and the art
# commissioned for those exact posts sat in src/assets/ referenced by nothing
# (ai-aikido, the-zombieagent-threat, why-deny-is-dangerous, fixed 2026-08-15).
# A placeholder IS a hero, so nothing here could see it.
#
# The rule that catches it: any asset which is not a generic placeholder must be
# referenced by something. Deliberately not a slug-to-filename match — those
# legitimately differ (permission-list-cant-see-taint uses permission-taint-gate,
# subagent-taint-experiment uses taint-subagent-boundary-16x9), and the zombieagent
# case would have slipped through a name match anyway.
#
# If this fires, either wire the asset to its post or delete it. An orphaned hero
# is someone's work that nobody can see.
for asset in src/assets/*.jpg src/assets/*.png src/assets/*.webp; do
  [ -e "$asset" ] || continue
  base="$(basename "$asset")"
  case "$base" in
    # Generic spares shipped with the Astro template; being unused is their job.
    blog-placeholder-*) continue ;;
    # Superseded by taint-subagent-boundary-16x9.jpg, which is wired. Kept rather
    # than deleted because it is the earlier take of the same concept.
    taint-subagent-boundary.jpg) continue ;;
  esac
  if ! grep -rq "$base" src/content src/pages src/components src/layouts 2>/dev/null; then
    echo "check:heroes: FAIL $base — dedicated asset referenced by nothing. Wire it to its post's heroImage, or delete it. (If it is deliberately spare, add it to the case list in this script with the reason.)" >&2
    fail=1
  fi
done

[ "$fail" -eq 0 ] && echo "check:heroes: OK — every post renders a hero ($(ls "$DIST" | wc -l) pages), and no dedicated asset is orphaned"
exit "$fail"
