## FEATURE:

Create one scoped improvement for x3-chain-master using the PRP workflow.
Suggested first target: automate a repeatable health check script that validates node startup prerequisites, key env files, and critical service ports.

## EXAMPLES:

- Use existing scripts in `scripts/` and top-level `run-*.sh` files as style references.
- Reuse validation/report patterns from `validate-test-framework.sh` and `RUN_ALL_TESTS.sh` where applicable.

## DOCUMENTATION:

- Project operational docs: `DEVELOPMENT.md`, `CONFIG.md`, `NODE_REQUIREMENTS.md`
- Deployment references: `X3_DEPLOYMENT_SOP.md`, `X3_GOLIVE_CHECKLIST.md`

## OTHER CONSIDERATIONS:

- Keep the first PRP small and high-confidence.
- Prefer additive changes; avoid broad refactors.
- Include explicit validation commands that can run locally.
