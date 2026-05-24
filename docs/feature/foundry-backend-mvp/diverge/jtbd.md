# JTBD — Foundry Backend MVP

## Raw Request

> "When my team needs an issue tracker that matches Linear's speed and workflow but is self-hostable and license-friendly, help me run one in under an hour on my own infrastructure so I can own my team's data and customize agentic workflows without paying per-seat."

## Job Extraction — 5 Whys

1. **Why an issue tracker?** — Engineering teams need a shared, low-friction system of record for "what we are doing this week" that survives chat-window churn and PR-comment scatter.
2. **Why match Linear?** — Linear set a new bar for input speed and keyboard-first flow; reverting to JIRA-class friction is intolerable to teams that have used it.
3. **Why self-host?** — Three distinct pressures stack: (a) per-seat economics on a 20+ person team rapidly exceed a single self-host VM, (b) data sovereignty/IP concerns for regulated or sovereign-cloud customers, (c) the customization ceiling on SaaS (no agentic workflows, no schema extensions).
4. **Why under an hour?** — If a self-host migration costs more than a quarter of seat fees, the team will not switch. "Hour-to-trial" is the credible-evaluation threshold for OSS infra in 2026.
5. **Why agentic workflows specifically?** — The 2025-2026 wave of dev-team AI (PR triage bots, design-doc generators, status synthesizers) needs a writable, queryable issue substrate. SaaS trackers gate this behind paid APIs and rate limits. The job-to-be-done is shifting: the tracker is becoming the agent's workspace, not just the human's.

## Abstraction Layer

- **Tactical** (rejected): "I want issues to load in 100ms." — Symptom, not job.
- **Operational** (rejected): "I want to migrate off Linear without losing my workflow." — A trigger, not a job; a job persists after migration.
- **Strategic** (accepted): "Help me operate my team's planning loop on infrastructure I control, with the speed and feel my team already trusts, so I can extend it (agentic workflows, custom fields, integrations) without vendor permission."
- **Physical**: "Reduce coordination cost between humans and machines collaborating on software, while keeping the state of that collaboration on hardware the team owns."

The strategic layer is the operative job for this MVP. The physical layer is the disruption frame (see below).

## Job Statements

**Functional**: When my team needs to coordinate work on a software product, I want a fast issue/project tracker running on infrastructure I control, so I can plan, execute, and report without per-seat fees or vendor lock-in.

**Emotional**: I want to feel that my team's planning data and workflow knowledge are my asset, not a vendor's lever — and that I can leave any tool, on any day, without an extraction tax.

**Social**: I want my team to perceive the tool as "as good as Linear" so adoption is voluntary, and I want to be seen as a technical leader who can choose pragmatic open-source infrastructure without sacrificing UX quality.

## Disruption Check

Is there a higher-level job that would make this whole job unnecessary?

**Candidate disruption**: "Eliminate the issue tracker entirely — agents pick work straight from a goal graph maintained by a chat thread."

**Verdict**: Premature. The 2026 evidence is that even AI-heavy teams still need an explicit, queryable, human-readable substrate to coordinate on. Agents are *consumers* of the tracker (and increasingly *writers* into it), not replacements for it. The job persists. This actually reinforces the strategic case for Foundry: the tracker becomes more important, not less, in the agentic era — which makes vendor lock-in more painful.

## ODI Outcome Statements

| # | Outcome | Importance (est.) | Satisfaction by Linear/JIRA/SaaS today | Opportunity Score | Status |
|---|---------|------------------|----------------------------------------|-------------------|--------|
| 1 | Minimize the time it takes to stand up a working issue tracker for a new team | 9 | 3 (Linear: minutes but SaaS only; self-host options today: hours-to-days) | 15 | **Under-served** |
| 2 | Minimize the likelihood that team planning data is held by a third party the team cannot extract from on demand | 9 | 4 (export exists but is degraded; workflow + automations don't export) | 14 | **Under-served** |
| 3 | Minimize the time it takes for a developer-contributor to make a meaningful change to the tracker itself | 8 | 2 (proprietary trackers: impossible; OSS trackers: large Python/Ruby/PHP codebases with hours-of-onboarding) | 14 | **Under-served** |
| 4 | Minimize the likelihood that issue-list interactions feel slower than Linear's | 9 | 5 (Linear is the bar) | 13 | **Under-served** (because OSS alternatives miss this) |
| 5 | Minimize the effort required to expose tracker state to an agentic workflow runner | 8 | 3 (Linear API is good but rate-limited and paid; JIRA API is ugly) | 13 | **Under-served** |
| 6 | Minimize the likelihood that running multiple replicas of the tracker requires a from-scratch ops investment | 7 | 4 (some OSS options are single-binary; most have explicit "use Redis + nginx + cron + worker" tax) | 10 | Appropriately served (but easy to lose) |
| 7 | Minimize the time it takes to upgrade between minor versions without breaking customizations | 7 | 4 | 10 | Appropriately served |

**Under-served outcomes (1-5) define the design directions.** Outcomes 6 and 7 are guardrails — directions must not regress against them.

## Refined Job Statement (for downstream waves)

> When an engineering team of 5-50 people needs to coordinate software work and is unwilling to keep paying per-seat fees or to surrender data sovereignty, help them run a Linear-feeling tracker on infrastructure they control, in under an hour, with a codebase a single Rust developer can understand in a day and extend (including with agentic workflows) without forking the world.

This refinement is what the brainstorming and taste phases operate on.
