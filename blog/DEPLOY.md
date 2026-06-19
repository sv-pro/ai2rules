# Deploying the blog (ai2rules.dev)

The **CLI Agent Harness** blog is a static [Astro](https://astro.build) site in
`blog/`. It deploys to **Cloudflare Pages** (hosting) and is served from
**ai2rules.dev** (registered at **Namecheap**). Registrar and host are
deliberately separate vendors.

> Branding stays **"CLI Agent Harness"** (`SITE_TITLE` in `src/consts.ts`); the
> domain is just the home.

---

## Already wired (in the repo)

- `astro.config.mjs` → `site: 'https://ai2rules.dev'` (canonical URLs, RSS,
  sitemap, and absolute OpenGraph/JSON-LD URLs all resolve to it).
- `.nvmrc` → `22.12.0` (Astro 6 needs Node ≥ 22.12; pins the build runtime).
- SEO layer in `src/components/BaseHead.astro`: `Article`/`TechArticle` JSON-LD,
  `robots: max-image-preview:large`, per-post OpenGraph/Twitter cards.

So deployment is account/dashboard work only — nothing left to change in code to
go live.

---

## Prerequisites

- A **Cloudflare** account (free plan is enough).
- Access to **ai2rules.dev** in **Namecheap** (to change nameservers).
- A **Google Search Console** account (for indexing / Discover eligibility).

---

## 1. Cloudflare Pages — connect & build

Dashboard → **Workers & Pages → Create → Pages → Connect to Git** → authorize
**GitLab** → select `sv-pro/cli-agent`.

Build settings (the subdirectory values matter — the repo root is the Rust
workspace, not the site):

| Setting                 | Value                                            |
| ----------------------- | ------------------------------------------------ |
| Production branch       | `dev` (or `main` — see *Branch strategy* below)  |
| Framework preset        | Astro                                            |
| **Root directory**      | `blog`                                            |
| Build command           | `npm run build`                                  |
| Build output directory  | `dist`                                            |

`.nvmrc` selects Node automatically. If a build ever grabs an older Node, also
set an environment variable `NODE_VERSION=22.12.0`.

Save → the first deploy produces a `*.pages.dev` preview URL. Sanity-check it
before attaching the domain.

---

## 2. Point ai2rules.dev at Pages (Namecheap → Cloudflare DNS)

The robust path for an apex domain is to let Cloudflare run DNS:

1. **Cloudflare → Add a site →** `ai2rules.dev` → Free plan. It returns **two
   nameservers**.
2. **Namecheap → Domain List → ai2rules.dev → Manage → Nameservers → Custom
   DNS** → paste the two Cloudflare nameservers → save. (Propagation: minutes to
   a few hours.)
3. **Cloudflare Pages → your project → Custom domains → Set up a domain →**
   `ai2rules.dev` (add `www.ai2rules.dev` too if desired). With DNS on
   Cloudflare, the records (apex via CNAME flattening) and HTTPS are provisioned
   automatically.

Result: `https://ai2rules.dev` live with automatic TLS.

> **Keeping DNS at Namecheap instead:** possible but messier — Namecheap has no
> proper apex `ALIAS`/`ANAME`, so you'd `CNAME` `www` to the `*.pages.dev` target
> and redirect the apex to `www`. Moving nameservers to Cloudflare avoids this.

---

## 3. Get it indexed (unlocks Google Discover)

- **Google Search Console** → add `ai2rules.dev` as a **Domain property** →
  verify with the DNS `TXT` record (easy once DNS is on Cloudflare) → submit
  `https://ai2rules.dev/sitemap-index.xml`.
- (Optional) Repeat in **Bing Webmaster Tools**.

After deploy, spot-check that `/`, `/blog`, `/rss.xml`, and
`/sitemap-index.xml` load, and that a post's HTML source contains the
`TechArticle` JSON-LD and a `canonical` link.

---

## Local build & preview

Requires Node ≥ 22.12 (see `.nvmrc`).

```bash
cd blog
npm install        # first time only
npm run dev        # local dev server at http://localhost:4321
npm run build      # production build into ./dist
npm run preview    # serve the built ./dist locally
```

---

## Branch strategy

Pages currently deploys from whichever branch you set as *production*. For a
public site, consider merging `dev → main` and setting **`main`** as the
production branch; pushes to `dev` then get automatic **preview** deployments
(their own URLs) without touching production.

---

## Backlog (optional — none block launch)

- Confirm the author display name in `src/consts.ts` (`SITE_AUTHOR`).
- E12.2 leftovers: WebSub pinging, automated OG-image generation, AVIF output.
- Recompress / rename the ~1 MB hero source JPGs under `src/assets/`
  (`blog-placeholder-*`); served output is already optimized to small WebP.
- Add the architectural diagram to the "Deny vs Absent" post (E12.3).
