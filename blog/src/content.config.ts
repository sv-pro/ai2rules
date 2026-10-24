import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';
import { z } from 'astro/zod';

const blog = defineCollection({
	// Load Markdown and MDX files in the `src/content/blog/` directory.
	loader: glob({ base: './src/content/blog', pattern: '**/*.{md,mdx}' }),
	// Type-check frontmatter using a schema
	schema: ({ image }) =>
		z.object({
			title: z.string(),
			description: z.string(),
			// Transform string to Date object
			pubDate: z.coerce.date(),
			updatedDate: z.coerce.date().optional(),
			// Unpublished: excluded from the listing, the feed, the sitemap and
			// getStaticPaths, so no page is generated at all. Still visible in
			// `astro dev` so a draft can be previewed. Default false, so existing
			// posts are unaffected.
			draft: z.boolean().default(false),
			heroImage: z.optional(image()),
		}),
});

export const collections = { blog };
