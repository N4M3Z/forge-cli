#!/usr/bin/env bash
set -euo pipefail

# forge-cli module validation script
# Canonical source: https://github.com/N4M3Z/forge-cli/blob/main/scripts/validate.sh
# Runs the same checks as `forge validate` without requiring the compiled binary.

UPSTREAM_URL="https://raw.githubusercontent.com/N4M3Z/forge-cli/main/scripts/validate.sh"
MODULE_ROOT="${1:-.}"
ERRORS=0

cd "$MODULE_ROOT"

# --- Drift detection ---

check_drift() {
    if ! command -v curl >/dev/null 2>&1; then
        return
    fi

    local local_hash
    local_hash=$(shasum -a 256 "$0" 2>/dev/null | cut -d' ' -f1)
    local upstream_content
    upstream_content=$(curl -sfL "$UPSTREAM_URL" 2>/dev/null || true)

    if [ -z "$upstream_content" ]; then
        return
    fi

    local upstream_hash
    upstream_hash=$(echo "$upstream_content" | shasum -a 256 | cut -d' ' -f1)

    if [ "$local_hash" != "$upstream_hash" ]; then
        echo "  DRIFT bin/validate.sh differs from upstream forge-cli"
    fi
}

# --- Module structure ---

check_required_files() {
    local required_files=(module.yaml defaults.yaml README.md LICENSE)

    for file in "${required_files[@]}"; do
        if [ -f "$file" ]; then
            echo "  ok $file"
        else
            echo "  MISSING $file"
            ERRORS=$((ERRORS + 1))
        fi
    done

    local optional_files=(INSTALL.md CONTRIBUTING.md CODEOWNERS CHANGELOG.md .gitattributes)

    for file in "${optional_files[@]}"; do
        if [ -f "$file" ]; then
            echo "  ok $file"
        else
            echo "  MISSING $file (optional)"
        fi
    done
}

# --- YAML validity ---

check_yaml_validity() {
    for file in module.yaml defaults.yaml; do
        if [ ! -f "$file" ]; then
            continue
        fi

        if command -v python3 >/dev/null 2>&1; then
            if python3 -c "
import sys
try:
    import yaml
    yaml.safe_load(open('$file'))
except ImportError:
    pass
except Exception as e:
    print(f'  INVALID $file: {e}')
    sys.exit(1)
" 2>/dev/null; then
                :
            else
                ERRORS=$((ERRORS + 1))
            fi
        fi
    done
}

# --- ADR frontmatter ---

check_adr_frontmatter() {
    if [ ! -d docs/decisions ] || [ ! -f bin/validate-adr.py ]; then
        return
    fi

    local schema=""
    for candidate in templates/forge-adr.json templates/structured-madr.json; do
        if [ -f "$candidate" ]; then
            schema="$candidate"
            break
        fi
    done

    if [ -z "$schema" ]; then
        return
    fi

    if command -v python3 >/dev/null 2>&1; then
        echo "  ADR frontmatter validation"
        if ! python3 bin/validate-adr.py "$schema" docs/decisions/; then
            ERRORS=$((ERRORS + 1))
        fi
    fi
}

# --- Shell lint ---

check_shell_lint() {
    local shell_files
    shell_files=$(find . -name '*.sh' -not -path '*/build/*' -not -path '*/target/*' -not -path '*/node_modules/*' 2>/dev/null || true)

    if [ -z "$shell_files" ]; then
        return
    fi

    if command -v shellcheck >/dev/null 2>&1; then
        echo "  shellcheck"
        echo "$shell_files" | xargs shellcheck -S warning 2>/dev/null || ERRORS=$((ERRORS + 1))
    else
        echo "  SKIP shellcheck (not installed)"
    fi
}

# --- Rust checks ---

check_rust() {
    if [ ! -f Cargo.toml ]; then
        return
    fi

    if ! command -v cargo >/dev/null 2>&1; then
        echo "  SKIP Rust checks (cargo not installed)"
        return
    fi

    echo "  cargo fmt --check"
    if ! cargo fmt --check 2>/dev/null; then
        ERRORS=$((ERRORS + 1))
    fi

    echo "  cargo clippy"
    if ! cargo clippy -- -D warnings 2>/dev/null; then
        ERRORS=$((ERRORS + 1))
    fi
}

# --- Python lint ---

check_python() {
    local python_files
    python_files=$(find . -name '*.py' -not -path '*/build/*' -not -path '*/target/*' -not -path '*/.venv/*' 2>/dev/null || true)

    if [ -z "$python_files" ]; then
        return
    fi

    if command -v ruff >/dev/null 2>&1; then
        echo "  ruff check"
        if ! ruff check . 2>/dev/null; then
            ERRORS=$((ERRORS + 1))
        fi
    else
        echo "  SKIP ruff (not installed)"
    fi
}

# --- TypeScript checks ---

check_typescript() {
    local typescript_files
    typescript_files=$(find . -name '*.ts' -o -name '*.tsx' 2>/dev/null | head -1 || true)

    if [ -z "$typescript_files" ]; then
        return
    fi

    if command -v npx >/dev/null 2>&1 && [ -f tsconfig.json ]; then
        echo "  tsc --noEmit"
        if ! npx tsc --noEmit 2>/dev/null; then
            ERRORS=$((ERRORS + 1))
        fi
    fi
}

# --- Run all checks ---

check_drift
check_required_files
check_yaml_validity
check_adr_frontmatter
check_shell_lint
check_rust
check_python
check_typescript

if [ "$ERRORS" -gt 0 ]; then
    echo ""
    echo "  $ERRORS errors found"
    exit 1
fi
