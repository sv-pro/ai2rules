# D70 — The publish path is rehearsed on every pull request, from a shared script


**Context.** `release.yml` runs only on a tag. Everything after the compile — the artifact
round-trip, unpacking four archives, filling the platform packages, packing the tarballs — sits
behind `if: startsWith(github.ref, 'refs/tags/')` and therefore executes for the first time on a
tag push, inside the job that publishes to npm. An npm version cannot be republished, so first
contact with a bug happens at the one point in the pipeline that has no undo.

That path had already produced two incidents, both caught by a human dry-running steps by hand
rather than by anything in the repository: GNU tar cannot read a zip (the Windows package would
have shipped empty), and `v0.3.1` went out as two platforms of four. A third was latent — see
below.

The proximate trigger was Dependabot. `actions/upload-artifact` and `actions/download-artifact`
appear **only** in `release.yml`, so the PRs bumping them across major versions arrived carrying a
complete set of green checks that could not, even in principle, have failed. Merging on green
would have been the exact mistake `blog/…/every-check-was-green` is about, in the week it was
published.

**Decision.** The assembly step becomes `scripts/assemble-npm-packages.sh`, called by both
`release.yml` and a new `release-dry-run` job in `ci.yml`. The CI job builds one real binary,
produces all four archives in the release's layout and names, uploads them as four separate
artifacts, **deletes the local copies**, downloads them back with `merge-multiple: true`, and then
runs the release's own steps in order. Deleting before the download is the load-bearing detail: it
is what makes the round-trip the thing under test rather than a formality.

A second guard, `scripts/check-npm-pack.mjs`, asserts that the tarballs npm *would* publish
actually contain their payload.

**The shared script is the decision, not an implementation detail.** A copied step would drift
from the one it rehearses, and a check that has drifted from the thing it checks is this
repository's most reliable failure mode — the rotted demos, the stale WASM, three tests asserting
a vulnerability. The rehearsal must be the performance.

**What it caught immediately, which is the argument for it.** Deleting the `files` allowlist from
a platform `package.json` — a plausible tidy-up — makes npm fall back to `.gitignore`, which
ignores `harness` and `LICENSE-*` because they are build outputs. The package then publishes
successfully, installs successfully, and has no binary. `npm/verify-packages.js` reports
**"npm layout OK"**. Verified by doing it: the existing guard passed, `check-npm-pack.mjs` failed
with "would publish WITHOUT harness".

- **Alternatives rejected.**
  - *`npm publish --dry-run` in CI.* Tried first, and it does not do what its name suggests: it
    contacts the registry and fails with "cannot publish over the previously published versions"
    at any version already on npm — which is every commit between releases. It would have been a
    permanently red job. `npm pack --dry-run` is local and covers the tarball contents, which is
    the whole delta. Recorded in the workflow so it is not helpfully re-added.
  - *Build all four targets in CI.* Three extra runners, including macOS and Windows, on every
    pull request, to test packaging rather than compilation. One real binary in four archives
    exercises the same code; `check` and `cross` own the compiler.
  - *Rely on `workflow_dispatch` against `release.yml`.* It exercises the build job only — the
    `npm` job is tag-gated too — and it requires someone to remember. A check nobody runs is the
    problem, not the solution.
  - *A draft GitHub release to exercise `softprops/action-gh-release`.* Rejected as the wrong
    trade: it creates real, visible releases on every pull request. **That action therefore stays
    unrehearsed, and this is a known residual** — it is the only step of the publish path this job
    does not cover.
- **Related:** D56, D58 (the npm layout this protects); `scripts/assemble-npm-packages.sh`,
  `scripts/check-npm-pack.mjs`, `.github/workflows/{ci,release}.yml`.

### D70 amendment (2026-08-15) — what the first candidate found, including about D70 itself

`v0.4.2-rc.1` was cut for one reason: `softprops/action-gh-release` runs only on a
tag, so D70's own residual said the only way to exercise it was a candidate. It was
exercised — four platform builds, eight assets, and the GitHub release correctly
marked as a prerelease. **The candidate then failed on `check-npm-pack.mjs`, one
step before publishing.** Nothing reached npm.

Two findings, and the second is the one worth keeping.

1. **`npm pack --json` has two output shapes.** npm ≤ 11 returns an array of
   entries; npm 12 returns an object keyed by package name. The release job runs
   `npm install -g npm@latest` because trusted publishing needs ≥ 11.5.1, while
   `release-dry-run` used whatever `setup-node` bundled. So the rehearsal ran npm 11
   and the performance ran npm 12. **This is the third instance of one rule** —
   after the shared assembly script and the action-pin parity guard — and the first
   where the drifting component was a *tool* rather than a file in this repository.
   The dry-run now upgrades npm the same way.

2. **The guard could not tell "I cannot read the tool" from "the package is
   broken."** Handed output it did not understand, it reported *every file missing
   from every package* — a verdict about the artifact, when the truth was that it
   had failed to inspect the artifact. **That is the exact confusion this project
   published a post about, pointed the other way.** There, a failure to *record* a
   decision was treated as permission to proceed; here, a failure to *reach* one was
   treated as a defect. Both come from the same place: at the call site, "no answer"
   and "a negative answer" have the same shape, and only one of them is about the
   thing under test.

   An unrecognised shape now throws, naming the npm version and the observed keys,
   labelled as an instrument failure. **A guard that cannot say "I do not know" will
   eventually say something false with confidence** — and in this case the false
   thing was severe enough to block a release, which is the harmless direction. The
   same bug in a guard that failed open would have published four empty packages.

**Kept, deliberately: `v0.4.2-rc.1`'s tag and GitHub prerelease stay.** They are a
valid, complete GitHub release that simply never reached npm, and re-pushing a
published tag to tidy the history is a worse habit than an honest gap in the version
sequence. `v0.3.1` was deleted and re-cut under a different rule — it was about to
become `latest`.
