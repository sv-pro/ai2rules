import type { CollectionEntry } from 'astro:content';

/**
 * Should this post be built and listed?
 *
 * Drafts are hidden from the production build entirely — no page, no listing
 * entry, no feed item, no sitemap URL — but remain visible under `astro dev` so
 * they can be previewed while being written.
 *
 * Import this everywhere rather than repeating `!data.draft`: the listing, the
 * feed and `getStaticPaths` must agree, and a draft that is unlisted but still
 * reachable at its URL is worse than one that is simply published.
 */
export const isPublished = ({ data }: CollectionEntry<'blog'>): boolean =>
	import.meta.env.DEV || !data.draft;
