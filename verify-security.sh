#!/bin/bash
# Security Verification Script
# Run this before making the repository public

set -e

echo "🔒 Breadboard Security Verification"
echo "===================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PASS="${GREEN}[PASS]${NC}"
FAIL="${RED}[FAIL]${NC}"
WARN="${YELLOW}[WARN]${NC}"

errors=0
warnings=0

# Check 1: No .env file in git
echo -n "Checking for .env files in git... "
if git ls-files | grep -qE '\.env$'; then
    echo -e "$FAIL"
    echo "  Found .env in git! Remove it:"
    git ls-files | grep -E '\.env$' | sed 's/^/  - /'
    ((errors++))
else
    echo -e "$PASS"
fi

# Check 2: No database files in git
echo -n "Checking for database files in git... "
if git ls-files | grep -qE '\.db$'; then
    echo -e "$FAIL"
    echo "  Found .db files in git! Remove them:"
    git ls-files | grep -E '\.db$' | sed 's/^/  - /'
    ((errors++))
else
    echo -e "$PASS"
fi

# Check 3: No hardcoded secrets
echo -n "Scanning for hardcoded secrets... "
if git grep -i "sk-[a-zA-Z0-9]\{20,\}" | grep -qv "example\|README\|SETUP\|SECURITY\|PRE_RELEASE"; then
    echo -e "$FAIL"
    echo "  Found potential secrets:"
    git grep -i "sk-[a-zA-Z0-9]\{20,\}" | grep -v "example\|README\|SETUP\|SECURITY\|PRE_RELEASE" | sed 's/^/  - /'
    ((errors++))
else
    echo -e "$PASS"
fi

# Check 4: cargo audit installed
echo -n "Checking cargo-audit installation... "
if ! command -v cargo-audit &> /dev/null; then
    echo -e "$WARN"
    echo "  cargo-audit not installed. Run: cargo install cargo-audit"
    ((warnings++))
else
    echo -e "$PASS"

    # Run cargo audit
    echo -n "Running cargo audit... "
    if cargo audit --deny warnings &> /dev/null; then
        echo -e "$PASS"
    else
        echo -e "$FAIL"
        echo "  Dependencies have vulnerabilities:"
        cargo audit | tail -20
        ((errors++))
    fi
fi

# Check 5: Build succeeds
echo -n "Testing build... "
if cargo build --release &> /dev/null; then
    echo -e "$PASS"
else
    echo -e "$FAIL"
    echo "  Build failed! Check errors:"
    cargo build --release 2>&1 | tail -10
    ((errors++))
fi

# Check 6: Tests pass
echo -n "Running tests... "
if cargo test &> /dev/null; then
    echo -e "$PASS"
else
    echo -e "$FAIL"
    echo "  Tests failed!"
    ((errors++))
fi

# Check 7: Security files exist
echo -n "Checking security documentation... "
missing_files=()
[ ! -f "SECURITY.md" ] && missing_files+=("SECURITY.md")
[ ! -f "PRE_RELEASE_CHECKLIST.md" ] && missing_files+=("PRE_RELEASE_CHECKLIST.md")

if [ ${#missing_files[@]} -gt 0 ]; then
    echo -e "$WARN"
    echo "  Missing files:"
    printf '  - %s\n' "${missing_files[@]}"
    ((warnings++))
else
    echo -e "$PASS"
fi

# Check 8: .gitignore complete
echo -n "Checking .gitignore... "
missing_patterns=()
grep -q "^\*\.db$" .gitignore || missing_patterns+=("*.db")
grep -q "^\.env$" .gitignore || missing_patterns+=(".env")
grep -q "^\*\.key$" .gitignore || missing_patterns+=("*.key")

if [ ${#missing_patterns[@]} -gt 0 ]; then
    echo -e "$WARN"
    echo "  Missing patterns in .gitignore:"
    printf '  - %s\n' "${missing_patterns[@]}"
    ((warnings++))
else
    echo -e "$PASS"
fi

# Check 9: No large files
echo -n "Checking for large files... "
large_files=$(git ls-files | xargs ls -lh 2>/dev/null | awk '$5 ~ /M$/ && $5+0 > 1 {print $9, $5}')
if [ -n "$large_files" ]; then
    echo -e "$WARN"
    echo "  Large files found (>1MB):"
    echo "$large_files" | sed 's/^/  - /'
    ((warnings++))
else
    echo -e "$PASS"
fi

# Summary
echo ""
echo "===================================="
if [ $errors -eq 0 ] && [ $warnings -eq 0 ]; then
    echo -e "${GREEN}✓ All checks passed! Repository is ready for public release.${NC}"
    exit 0
elif [ $errors -eq 0 ]; then
    echo -e "${YELLOW}⚠ $warnings warning(s) - Review before release${NC}"
    exit 0
else
    echo -e "${RED}✗ $errors error(s) and $warnings warning(s) - FIX BEFORE RELEASE${NC}"
    echo ""
    echo "Review the errors above and fix them before making the repository public."
    exit 1
fi
