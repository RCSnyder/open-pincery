---
description: "Deploy phase. Push to the deployment target, verify it's live, write README."
agent: "agent"
---

Deploy the verified software to its target.

## Steps

1. Read `scaffolding/scope.md` for deployment target
2. Read `scaffolding/design.md` for deployment config details
3. **Pre-flight check** — before attempting deploy, verify access for the target in scope.md:

   **Generic checks (all targets):**
   - [ ] Deploy CLI/tool is installed (`which <tool>`)
   - [ ] Authenticated to the deploy target (logged in, token available)
   - [ ] Deploy config file exists (e.g., `docker-compose.yml`, `Dockerfile`, GitHub Actions workflow, platform config)
   - [ ] Required env vars / secrets are set (check existence without printing values)
   - [ ] Build succeeds before attempting deploy

   **Platform-specific examples:**
   - **GitHub Pages**: `git remote -v` shows valid remote with push access
   - **Docker / VPS**: SSH access to host, Docker installed, `docker-compose.yml` exists, `.env` on host
   - **Container registry**: Registry credentials available in env, Dockerfile builds
   - **Binary release**: Build completes, artifact exists at expected path

   If any pre-flight check fails: STOP. Do not attempt deploy. Report what's missing.

4. **Identify the rollback command** before deploying. Write it down:

   ```
   If this deploy breaks, rollback command: [specific command]
   ```

   Examples:
   - **GitHub Pages**: `git push origin main` (re-triggers build from last good state)
   - **Docker / VPS**: `docker compose up -d --no-deps <service>` with previous image tag pinned in `.env` or compose file
   - **Container registry**: `docker tag <prev> <latest> && docker push` or redeploy previous image
   - **Binary release**: Previous release remains on GitHub — users can grab it

   The rollback command goes into DELIVERY.md under "Incident Response."

5. Deploy to the specified target:
   - **GitHub Pages**: Build, push to gh-pages branch or configure Actions
   - **Docker / VPS**: Build image, `docker compose up -d` on host (via SSH or CI)
   - **Container registry**: Build image, push, deploy
   - **Cron/script**: Set up the schedule, verify it runs
   - **Local/manual**: Document exact run commands
6. Verify it's accessible and working
7. Write `README.md` in the project root with:
   - What this is (one paragraph)
   - How to set up locally
   - How to deploy
   - How to run tests
8. Write `DELIVERY.md` in the project root. This is the client-facing handoff document. All projects get the same structure — depth scales naturally with the project's complexity.

### DELIVERY.md format

```markdown
# Delivery — [Project Name]

## What Was Built

[What it does, who it's for — 1-3 sentences]

## How to Use It

[Run command, URL, or access method]

## Acceptance Criteria — Verified

- [x] [Criterion 1] — Evidence: [how it was verified]
- [x] [Criterion 2] — Evidence: [...]

## Deferred Items

[Items from scope.md Deferred section. If none: "None — full scope delivered."]

## Known Limitations

[What it doesn't do, edge cases, performance bounds]

## Operational Notes

[How to monitor, restart, check logs, common issues. For simple tools: "Run [command]. No persistent state."]

## Architecture Overview

[Brief summary of how the system is structured. Point to scaffolding/design.md for full details.]

## Data & Migrations

[Current schema version, migration instructions, backup strategy. If stateless: "Stateless — no data persistence."]

## Security Posture

[Auth method, data handling, input validation, compliance notes. At minimum: "No secrets in code, HTTPS, parameterized queries."]

## Incident Response

[What to do when things break. Point to RUNBOOK.md if it exists. For simple tools: "Check logs, restart process, report issue."]

## Support Terms

[Bug-fix period, contact method, what's included vs. out of scope]

## Next Steps / Roadmap

[Recommended future work from deferred items, known limitations, and client input. If final: "No further work planned."]

## Version History

[v1 — date — summary of what was delivered]
[v2 — date — summary of iteration changes (if applicable)]
```

For simpler projects, sections will naturally be shorter (one-liners). The agent should not pad sections with filler — if a section is "N/A" or a single sentence, that's fine. The structure stays consistent so every delivery is navigable the same way.

## Post-Deploy Gate

- [ ] Deployed to specified target
- [ ] Accessible (can reach it, run it, or verify the cron fires)
- [ ] README.md exists with setup + run + deploy instructions
- [ ] DELIVERY.md exists with at minimum: what was built, how to use it, known limitations
- [ ] If stateful: data persistence verified (can create, read back)

If any gate condition fails, fix it and recheck. Up to 3 retries.

Log the result to `scaffolding/log.md` with URL/access method as evidence.

Git checkpoint:

```
git add -A && git commit -m "chore(deploy): deploy to [target]" -m "[URL or access method]\nGate: post-deploy PASS (attempt N)."
```

**STOP here and report to the user:**

```
✓ FULL PIPELINE COMPLETE.
[URL or access method]
[What was deployed and where]
[Summary of scaffolding/log.md — all phases, all gate results]

DELIVERY.md written with handoff details.
Scaffolding and input docs persist as project provenance.
To iterate: add feedback to docs/input/ and run /iterate.
```
