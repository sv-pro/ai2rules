#!/usr/bin/env bash
# CI guard: nothing marked `draft: true` may appear in the built site.
#
# Why this exists rather than trusting the filter: a post can surface in FOUR
# places — the listing, the RSS feed, the sitemap, and `getStaticPaths` — and each
# calls `getCollection()` itself. They are filtered by a shared predicate today
# (`src/published.ts`), but nothing forces a fifth query added later to use it.
# The failure is silent and one-directional: an unfiltered `getStaticPaths` builds
# a real, reachable page for a post you believe is unpublished. That is worse than
# publishing it outright, because it looks hidden while being fully readable.
#
# Astro removed built-in drafts in v3 precisely because the filtering belongs to
# the user's queries, so the user also owns this check.
#
# Run (from blog/): npm run build && npm run check:drafts
set -euo pipefail
cd "$(dirname "$0")/.."   # -> blog/

DIST="dist"
[ -d "$DIST/blog" ] || { echo "check:drafts: no $DIST/blog — run 'npm run build' first" >&2; exit 1; }

fail=0
drafts=0
published=0

for post in src/content/blog/*.md src/content/blog/*.mdx; do
  [ -e "$post" ] || continue
  slug="$(basename "$post")"; slug="${slug%.*}"

  # Match `draft: true` inside the frontmatter block only, so the word cannot
  # leak in from prose — a post *about* drafts must not mark itself one.
  if awk '/^---$/{n++; next} n==1' "$post" | grep -qE '^draft:[[:space:]]*true[[:space:]]*$'; then
    drafts=$((drafts + 1))

    # 1. no page generated (the one that matters: a reachable URL)
    if [ -e "$DIST/blog/$slug/index.html" ]; then
      echo "check:drafts: FAIL $slug — draft has a built page at $DIST/blog/$slug/index.html" >&2
      fail=1
    fi
    # 2. absent from the listing, the feed and the sitemap
    for surface in "$DIST/blog/index.html:listing" "$DIST/rss.xml:feed"; do
      f="${surface%%:*}"; label="${surface##*:}"
      if [ -f "$f" ] && grep -q "$slug" "$f"; then
        echo "check:drafts: FAIL $slug — draft appears in the $label ($f)" >&2
        fail=1
      fi
    done
    for sm in "$DIST"/sitemap*.xml; do
      [ -e "$sm" ] || continue
      if grep -q "$slug" "$sm"; then
        echo "check:drafts: FAIL $slug — draft appears in the sitemap ($sm)" >&2
        fail=1
      fi
    done
  else
    published=$((published + 1))
    # The inverse error is quieter and just as real: a typo'd `draft: ture`, or a
    # post someone meant to publish that silently never built.
    if [ ! -e "$DIST/blog/$slug/index.html" ]; then
      echo "check:drafts: FAIL $slug — published post has no built page" >&2
      fail=1
    fi
  fi
done

if [ "$fail" -ne 0 ]; then
  echo "check:drafts: a draft reached the build, or a published post did not" >&2
  exit 1
fi

echo "check:drafts: OK — $published published, $drafts draft(s), none leaked"
