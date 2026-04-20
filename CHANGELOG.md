# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0](https://github.com/RCSnyder/open-pincery/releases/tag/v1.0.0) - 2026-04-20

### Added

- *(cli)* add 'pcy demo' for one-command end-to-end smoke test
- *(auth)* add /api/login endpoint for session token recovery
- *(build)* v5 operator onramp (AC-28..AC-33)
- *(build)* v5 slice 1+2 compose + .env.example rewrite with regression tests
- *(build)* v4 slice 5 deliver vanilla JS control plane (AC-26)
- *(build)* v4 slice 4 add pcy CLI binary and shared API client (AC-25)
- *(build)* v4 slice 3 add webhook secret rotation endpoint (AC-24)
- *(build)* v4 slice 2 enforce budget cap at wake acquire (AC-23)
- *(build)* v4 slice 1 non-root runtime image (AC-22)
- *(hooks)* auto-rustfmt on edits + fmt-check gate before git commit
- *(build)* v3 slice 6 — signed release artifacts with SBOM (AC-20)
- *(build)* v3 slice 4-5 — CI workflow (AC-16) + operator runbooks (AC-21)
- *(build)* v3 slice 3 — Prometheus metrics (AC-18)
- *(build)* v3 slice 1-2 — JSON logging (AC-17) + health/ready split (AC-19)
- *(build)* implement v2 features AC-11 through AC-15
- *(build)* implement dashboard UI — bootstrap, agent management, event log
- *(build)* complete BUILD phase — all 15 tests pass, all 10 ACs covered
- *(build)* add tests for AC-4, AC-5, AC-7 + fix llm_call/projection schema alignment
- *(build)* implement full application skeleton - Slice 1 complete

### Fixed

- *(ci)* checkout main branch in release-plz (not detached HEAD)
- *(ci)* allow CDLA-Permissive-2.0 license in cargo-deny
- *(build)* address REVIEW v4 findings and finalize v4 BUILD state
- *(review)* address v3 review findings (1 Critical + 5 Required + 2 Consider)
- *(review)* address all Critical and Required review findings
- *(build)* resolve 7 audit findings from struct/migration mismatches and API gaps
- *(build)* address REVIEW findings — 2 critical, 6 required fixes

### Other

- *(release)* cut 1.0.0 and wire up automated releases
- *(input)* add improvement-ideas brainstorm
- *(build)* fix Docker build for Rust 1.88 toolchain
- *(deploy)* v5 delivery — log + DELIVERY.md finalized
- *(analyze)* v5 readiness — READY verdict
- *(design)* v5 design addendum — operator onramp contract
- *(expand)* v5 scope — operator onramp
- *(deploy)* v4 delivered — README, DELIVERY, log updated
- *(reconcile)* sync v4 scaffolding with shipped code
- *(build)* record v4 BUILD gate pass evidence
- *(build)* narrow sqlx features and refresh lockfile
- *(build)* add v4 API stability contract (AC-27)
- *(build)* add static Dockerfile guard for AC-22
- *(iterate)* v4 readiness — READY
- *(iterate)* v4 design — CLI/UI/safety integration points
- *(iterate)* v4 scope — usable self-host (CLI + UI + safety hardening)
- *(hooks)* split by concern + block destructive commands
- *(deploy)* v3 delivery — README + DELIVERY.md + log
- rustfmt wrap assert! in json logging test
- *(review)* log RECONCILE + REVIEW pass 2 PASS
- *(reconcile)* align design + readiness with v3 code post-review
- *(iterate)* v3 scope, design, readiness — observability and release hygiene
- *(deploy)* complete v2 delivery
- *(reconcile)* fix v2 scaffolding drift
- *(analyze)* v2 readiness.md — READY verdict, AC-11 through AC-15
- *(iterate)* version scope.md and design.md for v2 — operational readiness
- update README with accurate quick start and project structure
- *(deploy)* README + DELIVERY.md, post-deploy gate PASS
- *(verify)* post-verify gate PASS — all 10 ACs verified, 17/17 tests pass
- *(reconcile)* fix structural drift across scaffolding documents
- *(build)* add tests for AC-6, AC-8, AC-9
- *(build)* add integration tests for AC-1, AC-2, AC-3, AC-10
- *(analyze)* produce readiness.md — READY verdict
- *(design)* define architecture for Open Pincery v1
- *(expand)* define scope for Open Pincery v1 agent runtime
- rewrite README for open-pincery and update LICENSE copyright
- *(input)* add arch pdf, tla+ spec, and additional arch .md files
- Initial commit
