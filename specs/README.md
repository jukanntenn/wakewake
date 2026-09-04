# WakeWake Specs

English | [中文](README.zh.md)

Design specs committed to the repository. The rules:

- Specs describe **mechanisms and invariants** (why it is designed this way, what must not move); operational procedures belong in [`../devops/README.md`](../devops/README.md), and code commentary belongs in the source.
- When the implementation changes, the matching spec changes with it; spec/implementation drift is a bug.
- Adding a spec file means adding its row here in the same change ([`scripts/verify_specs_index.py`](../scripts/verify_specs_index.py) enforces both directions); decision history lives in [`.agents/wrfcs/`](../.agents/wrfcs/README.md).

## Index

| File | What it covers |
|---|---|
| [`backend/agent-distribution.md`](backend/agent-distribution.md) | Agent distribution: GitHub Releases binaries + Docker Hub image, the install.sh one-liner contract, the `service install` subcommand, `tls.ca_cert` for self-signed TLS, the `--network host` requirement |
| [`backend/configuration.md`](backend/configuration.md) | The configuration mechanism: three-layer override, server/agent schemas, env mapping, example-file conventions, the per-environment deployment map, and invariants |
| [`backend/risk-controls.md`](backend/risk-controls.md) | Admin runtime risk controls: the runtime-handle family (maintenance/mailer/pow/ip-bans), per-path email budgets and their degradation semantics, unverified-account purge invariants, IP-ban matching and fail-open rules, the risk panel's data sources |
| [`frontend/agent-onboarding.md`](frontend/agent-onboarding.md) | Agent-onboarding three-state page + device-add pre-flight gating: quota via `UserPublic.limits`, pending/offline semantics, command template slots, secure-context interception |
| [`testing/load.md`](testing/load.md) | Load and capacity testing: the two rigs (bare-metal / prod-sim), the 2c2g3Mbps environment contract with egress shaping, the R1–R7 scenario matrix, the seed data contract, red lines, and the knee/sweet-spot judgement methodology |
