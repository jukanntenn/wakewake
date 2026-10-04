---
name: shipping
description: Ship the current work end-to-end — commit, build & push the image, deploy, report. Use when the user asks to ship, publish, deploy, or release the current changes.
disable-model-invocation: true
---

Ship the current work in one pass: commit → build & push → deploy → report.

1. **Gate.** From the repo root, run the same prek gates CI runs: `prek run --all-files --group format --group lint` (whole workspace), then `prek run --all-files --hook-stage pre-push backend:test` and `frontend:test` — backend tests need a live PostgreSQL (`python3 devops/dev.py init` brings one up). Any failure: stop, fix, re-run until green.
2. **Commit.** Delegate to the commit skill.
3. **Build & push.** `python3 docker/build.py --push` — unified image, default `main` tag (the `test` environment tracks `main`). Extra SemVer tags only if the user names one: `--tags 0.1.3-rc.1` (hyphen required — `v0.1.3rc1` is illegal).
4. **Deploy.** `ansible-playbook devops/ansible/deploy.yml -l test` by default; `staging`/`prod` only if the user names one. The playbook's post-deploy health + git-sha verification owns the outcome — do not re-verify by hand.
5. **Report.** Image reference, environment, outcome — one line.
