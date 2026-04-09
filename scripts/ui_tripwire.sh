#!/usr/bin/env bash
# UI tripwire: fails on regressions against the v2.0.0 redesign commitments.
#
# Non-negotiable rules enforced here:
#   1. No `Mode::` references anywhere — phase 2 replaced them with Focus + Overlay.
#   2. No `cursor_pos` on App — phase 3 replaced `input: String + cursor_pos` with
#      `input: TextBuffer`.
#   3. `Borders::ALL` only in chrome.rs, modal.rs, and the pre-TUI password modal
#      in main.rs. Every pane goes through `chrome::panel`.
#   4. No `Color::*` in the theme-wired files (chat_list, hintbar, welcome, chrome).
#      statusline is exempt because its pill backgrounds (red reconnect, yellow
#      locked, green/red balance deltas) are semantic — not theme palette.

set -eu

cd "$(dirname "$0")/.."

fail=0

grep_nonzero() {
    local label="$1"
    local pattern="$2"
    shift 2
    local found
    if found="$(grep -nE "$pattern" "$@" 2>/dev/null)"; then
        echo "[FAIL] $label"
        echo "$found"
        fail=1
    fi
}

grep_nonzero "Mode:: still referenced" \
    '\bMode::' \
    $(find src -name '*.rs' -not -path 'src/ui/overlay/help.rs' 2>/dev/null || true)

grep_nonzero "cursor_pos on App should be replaced by TextBuffer" \
    '\bcursor_pos\b' \
    src/app.rs src/main.rs 2>/dev/null || true

grep_nonzero "Borders::ALL outside chrome/modal/password-modal" \
    'Borders::ALL' \
    $(find src -name '*.rs' ! -name chrome.rs ! -name modal.rs ! -name main.rs 2>/dev/null || true)

for f in src/ui/chat_list.rs src/ui/hintbar.rs src/ui/welcome.rs src/ui/chrome.rs; do
    if [ -f "$f" ] && grep -qE '\bColor::' "$f"; then
        echo "[FAIL] hardcoded Color:: in theme-wired file: $f"
        grep -nE '\bColor::' "$f"
        fail=1
    fi
done

echo
echo "informational (not gated): residual Color:: outside theme.rs"
rg -l '\bColor::' src/ui 2>/dev/null | grep -v 'theme.rs' | while read -r f; do
    count=$(grep -cE '\bColor::' "$f")
    printf "  %4d  %s\n" "$count" "$f"
done

if [ $fail -eq 0 ]; then
    echo
    echo "[OK] ui tripwire clean"
fi
exit $fail
