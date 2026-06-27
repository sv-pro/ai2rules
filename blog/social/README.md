# blog/social

Promotion copy for blog posts — the seeding step in
[`docs/BLOG_PLAN.md`](../../docs/BLOG_PLAN.md) §5 (Hacker News / Reddit / X).

This lives **outside `blog/src/`** on purpose: it's marketing source, not site
content, so Astro never renders or deploys it.

## Convention

- **One file per post**, named after the post slug:
  `blog/social/<post-slug>.md` ↔ `blog/src/content/blog/<post-slug>.md`.
- Each file carries, at minimum, an **X/Twitter thread** and a few interchangeable
  **standalone posts**, all linking to the canonical post URL
  (`https://ai2rules.dev/blog/<post-slug>/`).
- Keep each tweet body **≤ 280 chars**. Links count as 23 on X regardless of length
  — the char notes in each file already assume that.
- Add HN/Reddit titles here too when you write them; they're part of the same seed.
