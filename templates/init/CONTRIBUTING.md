# Contributing

## Getting Started

```sh
git clone https://github.com/OWNER/${MODULE_NAME}.git
cd ${MODULE_NAME}
make install
```

## Conventions

See [README.md](README.md) for project-specific conventions.

## Git

Conventional Commits: `type: description`. Lowercase, no trailing period, no scope.

Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`.

## Pull Requests

1. Fork and create a branch
2. Make changes following the conventions above
3. `make test`
4. Open a PR against `main`

CI runs validation on every PR. The `main` branch requires passing CI before merge.
