## FEATURE:

Bring all apps in `apps/` to "100% readiness" by making their quality gates pass consistently.

Target apps:
- `apps/dex`
- `apps/wallet`
- `apps/x3-desktop`
- `apps/x3-intelligence`
- `apps/validators`
- `apps/inferstructor-dashboard`

Definition of 100%:
- Lint passes
- Typecheck passes (if script exists)
- Tests pass (or are explicitly configured to pass with no tests where appropriate)
- Build passes

Current baseline source:
- `.artifacts/apps-quality-baseline.txt`

## EXAMPLES:

Use these repo examples/patterns:
- `apps/validators/package.json` (good script surface including `type-check`, `lint`, `build`)
- `apps/x3-desktop/package.json` + `apps/x3-desktop/src-tauri/` (desktop + tauri integration pattern)
- `apps/wallet/package.json` (next + jest setup pattern)
- root `package.json` and `jest.config.cjs` for monorepo script and test conventions

## DOCUMENTATION:

- Next.js lint/build docs:
	- https://nextjs.org/docs/app/building-your-application/configuring/eslint
	- https://nextjs.org/docs/app/api-reference/config/next-config-js
- TypeScript config best practices:
	- https://www.typescriptlang.org/tsconfig
- ESLint flat/classic config migration:
	- https://eslint.org/docs/latest/use/configure/
- Tauri app API package docs:
	- https://v2.tauri.app/reference/javascript/api/

## OTHER CONSIDERATIONS:

- Do not introduce UX/features outside readiness scope.
- Prefer smallest diff per app; avoid broad refactors.
- Preserve existing scripts unless broken; fix invocation/config first.
- For apps with intentionally no tests, align scripts to non-failing behavior (`--passWithNoTests`) only when justified.
- Keep changes compatible with current workspace toolchain and existing Tauri setup.
