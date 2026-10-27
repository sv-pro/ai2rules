# Pulse: the weekly "what now?"

This file **is** the prompt of the Pulse task (a local scheduled task in the Claude
desktop app, Sunday evening, Asia/Jerusalem; see
[Cadence](../FLYWHEEL.md#cadence-the-clock)). The task itself only says "follow
`docs/flywheel/pulse.md` from `origin/main`", so changing the Pulse means changing this
file in a PR.

Pulse runs on the maintainer's machine because it needs what a cloud routine lacks: the
desktop notification and the dashboard page. Its job is to look at the whole flywheel
once a week and answer one question so the maintainer never has to: **what is the next
thing to do?**

## Ground rules

- **Read-only on the repository.** The checkout at `C:\Users\imser\workspace\ai2rules`
  may be in the middle of someone else's work (another session, another branch, an
  uncommitted HANDOFF.md). Never check out, stash, commit, move or edit anything there.
  Read the board from `origin/main` with `git fetch`, `git ls-tree` and `git log`.
- **You report, you don't triage.** Never change the state of a finding, a task or a PR.
  The only things you create or change are the dashboard page, your own `[Pulse]` Linear
  issue (and closing the previous one), and one notification.
- **Write for the maintainer in Russian.** Keep file names, issue keys and titles as they
  are.

## 1. Gather

From the repository (`cd C:\Users\imser\workspace\ai2rules`, then `git fetch -q origin`):

- **Board.** For each column `_tasks/1_discovery`, `2_development`, `3_advocacy`,
  `4_done`: `git ls-tree --name-only origin/main _tasks/<column>/`. For every item outside
  `4_done`, its age = days since `git log -1 --format=%cs origin/main -- <path>`, and its
  first heading (`git show origin/main:<path>`, first `# ` line).
- **Done this week.** Items whose last move into `4_done/` falls in the last 7 days, and
  `git log --since="7 days ago" --merges --oneline origin/main` for merged PRs.
- **Open PRs.** `gh pr list --repo sv-pro/ai2rules --state open --limit 30 --json
  number,title,headRefName,isDraft,createdAt,updatedAt,reviewDecision,statusCheckRollup`.
  Summarise checks as green / red / pending.

From Linear (team **AI2**):

- **Untriaged findings:** label `radar`, state Backlog (with age since creation).
- **Accepted, not started:** label `radar`, state Todo.
- **In flight:** state In Progress or In Review.
- **Waiting on a decision:** open issues with label `decision`.
- The latest `[Radar digest] …` issue (date and link).

From the Radar routine (load `RemoteTrigger` with ToolSearch; skip this step if it is not
available): `list` → the routine named **Flywheel Radar** → `list_runs` → the latest
run's time and status.

## 2. Apply the rules

Flag, in this order of severity:

1. **Radar down:** no successful Radar run in the last 5 days, or the routine is disabled.
2. **Waiting on the maintainer only:** a non-draft PR with green checks and no review
   decision; an issue labelled `decision`.
3. **Untriaged:** `radar` findings in Backlog older than 7 days.
4. **WIP over limit:** more than **2** items in `_tasks/2_development/`, or more than 2
   open non-draft PRs.
5. **Stale:** any item in `1_discovery`, `2_development` or `3_advocacy` unchanged for
   more than **14 days**. Each needs one decision: do it now, park it in
   `docs/RESEARCH-BACKLOG.md`, or close it.

## 3. Choose ONE next step

Pick the single action with the most leverage, in this priority:

1. Fix Radar if it is down (the loop has no input otherwise).
2. Anything blocking other work that only the maintainer can do (review/merge a green PR,
   answer a `decision` issue).
3. Triage untriaged findings (one sitting, a few minutes).
4. Decide the oldest stale item (do / park / close).
5. Publish or seed something sitting finished in `3_advocacy/`.
6. Otherwise: start the top accepted (`Todo`) finding.

Phrase it as an instruction that fits in one line and names the exact item, e.g.
«Разбери 3 находки Radar в Backlog (AI2-110, AI2-111, AI2-113) — 5 минут». Then add up to
two runner-up steps.

## 4. Publish

### The dashboard page

Load the `artifact-design` skill first. Use the Artifact tool's `list` to find the page
titled **Flywheel Pulse**. If it exists, `read` it, then republish to its `url` from a
local HTML file in your scratchpad; if it does not, publish it new (icon `chart`). Do not
pin it.

The page, in Russian, in this order:

1. **Следующий шаг**: the one step, large, with the two runners-up under it.
2. **Тревоги**: the flags from step 2, most severe first (omit the section if none).
3. **Доска**: four columns (Discovery, Development, Advocacy, Done this week), each item
   with its age in days; ages over 14 shown as a warning.
4. **Radar**: last run (time, status), untriaged findings with links, accepted ones.
5. **PR**: open PRs with draft flag, checks and age.
6. A footer: «Обновлено <date time>, следующий Pulse: <next Sunday>».

### The Linear issue

Close the previous open `[Pulse] …` issue (state Done). Create
`[Pulse] <YYYY>-W<ISO week>`: team AI2, labels `flywheel`, assignee
`sergeyvlasov.pro@gmail.com`, state Todo, **due date: tomorrow**. Body: the same content
as the page in Markdown, with the page link at the top.

### The notification

Load `PushNotification` with ToolSearch and send one line, under 200 characters:
`Flywheel: <the one next step>`.

## 5. Finish

End with three lines: the next step, the number of flags, and the page link.
