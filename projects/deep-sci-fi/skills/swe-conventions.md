# SWE Conventions — Deep Sci-Fi

This skill document is auto-injected into SWE agent prompts via the ProjectHarness. It contains everything an SWE agent needs to work in the deep-sci-fi codebase.

## Repository Layout

```
deep-sci-fi/
├── platform/                          # Next.js 14 frontend (App Router, RSC, Bun)
│   ├── app/                           # App Router pages and layouts
│   ├── components/                    # Shared React components
│   ├── lib/                           # Utilities, hooks, client-side logic
│   ├── public/                        # Static assets
│   ├── tailwind.config.ts             # Tailwind configuration
│   ├── tsconfig.json                  # TypeScript config
│   ├── package.json                   # Frontend dependencies (Bun)
│   ├── vitest.config.ts               # Vitest configuration
│   ├── playwright.config.ts           # Playwright E2E configuration
│   ├── backend/                       # FastAPI backend
│   │   ├── app/                       # FastAPI application
│   │   │   ├── main.py               # App entry point
│   │   │   ├── models.py             # SQLAlchemy models
│   │   │   ├── schemas.py            # Pydantic response models
│   │   │   ├── api/                   # API route modules
│   │   │   └── core/                  # Config, database, dependencies
│   │   ├── alembic/                   # Alembic migrations
│   │   │   ├── versions/             # Migration files
│   │   │   └── env.py                # Migration environment
│   │   ├── tests/                     # pytest unit tests
│   │   │   └── simulation/           # Hypothesis DST tests
│   │   ├── requirements.txt          # Python dependencies
│   │   └── Dockerfile                # Backend container
│   └── e2e/                           # Playwright E2E test suites (12 specs)
├── scripts/                           # Harness enforcement scripts
│   ├── pre-commit                    # Level 1 gate checks
│   ├── pre-push                      # Level 2 gate checks
│   └── policy-check.sh              # Policy verification
├── .github/workflows/                 # CI/CD pipelines
│   ├── review.yml                    # PR review checks
│   ├── deploy.yml                    # Deployment pipeline
│   ├── post-deploy-verify.yml        # Post-deploy health checks
│   ├── feedback-fix.yml              # Automated feedback fixes
│   └── feedback-triage.yml           # Feedback triage
└── .vision/                           # Project vision documents
    ├── TASTE.md                      # Design system reference
    ├── SCIENTIFIC_GROUNDING.md       # Science accuracy standards
    └── WORLD_ASPECTS_MODEL.md        # World-building framework
```

## Common Pitfalls

- **App Router, not pages/.** Next.js 14 uses the `app/` directory for routing, not `pages/`. Don't create files in `pages/`.
- **Backend is inside platform/.** The FastAPI backend lives at `platform/backend/`, not at the repo root. All backend commands run from `platform/backend/`.
- **Bun, not npm.** The frontend uses Bun. Use `bun install`, `bun run`, `bunx`. Never `npm install` or `npx`.
- **Alembic, not raw SQL.** All schema changes go through Alembic migrations. Never modify the database directly.
- **response_model is required.** Every API endpoint must declare a Pydantic `response_model`. The policy gate checks for this.

## Testing Commands

```bash
# Backend unit tests
cd platform/backend && pytest tests/ -x -q

# Hypothesis DST (property-based stateful tests)
cd platform/backend && pytest tests/simulation/ -x

# Frontend unit tests
cd platform && bun run test:run

# Playwright E2E tests
cd platform && bun run test:e2e

# TypeScript type check
cd platform && bun run typecheck
```

Run ALL of these before pushing. The Level 2 pre-push gate will block you if DST or coverage checks fail.

## Database Migrations

### Creating migrations
```bash
cd platform/backend
alembic revision --autogenerate -m "description of change"
```

### Migration rules
- **UPPERCASE enums.** PostgreSQL ENUMs must use UPPERCASE values. Use `postgresql.ENUM` with `create_type=False`.
- **Idempotent migrations.** Always include existence checks (`IF NOT EXISTS`, `IF EXISTS`) so migrations can be re-run safely.
- **One migration per change.** Don't batch unrelated schema changes into one migration.
- **Test migrations.** Run `alembic upgrade head` and `alembic downgrade -1` to verify both directions work.

### When migrations are required
Any change to `models.py` requires a corresponding Alembic migration. The Level 1 pre-commit gate checks for this — if you change models without a migration, the commit is rejected.

## CI Gate Requirements

Before pushing, ensure all Level 1 + Level 2 checks pass:

1. **Migration check** — models.py changes have corresponding migrations
2. **Review markers** — `code-reviewed` and `dst-reviewed` markers present
3. **Skill.md sync** — API endpoint tables match actual routes
4. **DST coverage** — state-mutating endpoints have simulation tests
5. **Response model coverage** — changed API files declare `response_model`
6. **API test coverage** — changed API modules have route-prefix tests
7. **Frontend-E2E mapping** — changed frontend files have E2E specs (if user-facing)
8. **DST simulation** — `pytest tests/simulation/ -x` with seed=0 passes
9. **DST coverage gate** — no uncovered state-mutating endpoints

## Conventional Commits

```
feat: add world proposal voting endpoint
fix: correct pgvector similarity threshold for foresight worlds
refactor: extract embedding pipeline into standalone module
docs: update API endpoint table in SWE skill
chore: bump FastAPI to 0.115.0
```

Always use the appropriate prefix. PRs with non-conventional commit messages will be flagged.

## PR Workflow

1. Branch from `main`: `git checkout -b feat/description` or `fix/description`
2. Make changes, ensure all gates pass locally
3. Push: `git push -u origin feat/description`
4. Create PR: `gh pr create --base main --repo arni-labs/deep-sci-fi`
5. Wait for CI (review.yml) to pass
6. Request review from Ren (product lead)
7. Do NOT merge — only Ren or the human can merge PRs

## WorkCycle Integration

When working on a task tracked by a WorkCycle entity, report gate results as you go:

```
# After running migrations check
→ ReportMigrations(ok="true", summary="Alembic migration 0042 created for new column")

# After typecheck passes
→ ReportTypecheck(ok="true", summary="bun run typecheck clean, 0 errors")

# After unit tests pass
→ ReportUnitTests(ok="true", summary="pytest 47 passed, 0 failed")

# After DST passes
→ ReportDST(ok="true", summary="Hypothesis DST 200 examples, 0 failures")

# After all policy gates pass
→ ReportPolicyGates(ok="true", summary="All Level 1 + Level 2 gates green")

# After E2E (if applicable)
→ ReportE2E(ok="true", summary="Playwright 12/12 specs passed")
```

If any gate fails, report `ok="false"` with a description of the failure. The WorkCycle will not allow transition to Testing until the required gates are green.
