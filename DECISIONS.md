# Decision Log — moved

The decision log is now **one file per decision** under
[`decisions/`](decisions/), indexed at
**[`decisions/INDEX.md`](decisions/INDEX.md)**.

This file stays because 65 links across the repo, the docs and the published blog
point at `DECISIONS.md`. It is a signpost, not the log.

## Where things went

| You want | Go to |
|---|---|
| the whole list, with epics and statuses | [`decisions/INDEX.md`](decisions/INDEX.md) |
| one decision, e.g. D24 | `decisions/D024-*.md` |
| to add a decision | create `decisions/D<nnn>-<slug>.md`, then run `node scripts/build-decisions-index.mjs` |

Nothing was lost in the move: every entry's body is byte-identical to what this
file held, and the only change is that each `## D<n> — Title` heading became the
`# D<n> — Title` heading of its own file.

## Why it was split

At 212 KB this was the largest hand-edited file in the repository — the only one
big enough to exceed a tool's read budget. In [#59] something read it truncated
and wrote the truncation back: the file was an unreadable blob for three commits,
and 689 lines of entries were silently gone before the recovery in [#69]. The
forensics are in [#70].

A guard that rejects a corrupt file is worth having, and is tracked separately.
But the file was going to outgrow the next budget too, so the log was split
instead: no single file is near any limit now, and an edit touches one small file
rather than rewriting a quarter-megabyte of history.

[#59]: https://github.com/sv-pro/ai2rules/pull/59
[#69]: https://github.com/sv-pro/ai2rules/pull/69
[#70]: https://github.com/sv-pro/ai2rules/issues/70
