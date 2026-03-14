# CLAUDE.md

This file provides guidance to AI assistants (Claude, etc.) working with this repository.

## Repository Overview

**Name:** sync_code
**Owner:** CornPrincess
**License:** MIT
**Status:** Early-stage / initial setup

This repository is at its inception — currently containing only a `LICENSE` (MIT, 2026) and a `README.md` placeholder. No source code, tests, or configuration has been added yet.

## Repository Structure

```
sync_code/
├── CLAUDE.md       # This file — AI assistant guidance
├── LICENSE         # MIT License (2026 CornPrincess)
└── README.md       # Project title placeholder
```

## Git Workflow

### Branches
- `main` — primary default branch (on remote `origin`)
- `master` — local default branch
- Feature/task branches follow the pattern `claude/<description>-<id>` (e.g. `claude/add-claude-documentation-QW8E3`)

### Commit Conventions
- Use clear, imperative commit messages (e.g. `Add CLAUDE.md with project documentation`)
- Keep commits focused — one logical change per commit
- Reference issue/task IDs in commit messages when applicable

### Push Workflow
```bash
git push -u origin <branch-name>
```
- Branch names for AI-generated work must start with `claude/` and end with the session ID suffix

## Development Guidelines

Since the project has not yet defined a tech stack or source structure, the following conventions should be established as code is added:

### General Principles
- Keep the repository clean — avoid committing generated files, secrets, or build artifacts
- Add a `.gitignore` appropriate to the chosen tech stack before committing source code
- Document environment variables in a `.env.example` file (never commit `.env` with real secrets)

### When Adding Source Code
1. Update `README.md` with: project description, prerequisites, setup steps, and usage
2. Add a `.gitignore` matching the language/framework
3. Add dependency management files (`package.json`, `pyproject.toml`, `go.mod`, etc.)
4. Create a `src/` or similarly conventional source directory
5. Set up a test directory (`tests/`, `__tests__/`, `spec/`, etc.) from the start

### When Adding CI/CD
- Use `.github/workflows/` for GitHub Actions
- Include at minimum: lint, test, and build jobs
- Run checks on pull requests targeting `main`/`master`

## AI Assistant Instructions

- **Do not invent structure** — only describe or create what is actually present or explicitly requested
- **Prefer editing existing files** over creating new ones
- **Keep changes minimal and focused** — avoid over-engineering or adding unrequested features
- **Always commit on the correct branch** — check the active branch before making commits
- **Update this file** whenever significant new structure, conventions, or workflows are established

## Future Sections to Add

As the project evolves, expand this file with:
- Tech stack and language versions
- Setup and installation instructions
- How to run tests (`make test`, `npm test`, `pytest`, etc.)
- How to run linters/formatters and what tools are used
- Environment variable reference
- Architecture overview
- Deployment process
