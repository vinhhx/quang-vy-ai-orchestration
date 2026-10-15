#!/usr/bin/env bash
# S6 evidence check (completeness only; never judges the outcome). `--basic` checks only the basic-mode files.
set -u
DIR="$(cd "$(dirname "$0")" && pwd -P)"
R="$DIR/raw"
fail=0
chk() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then echo "ok   $d"; else echo "FAIL $d"; fail=1; fi; }
chk "stdout exists" test -f "$R/s6-stdout.txt"
chk "stderr has exit_code line" grep -qE '^exit_code=[0-9]+$' "$R/s6-stderr.txt"
chk "help has exit_code line" grep -qE '^exit_code=[0-9]+$' "$R/s6-dsh-help.txt"
chk "a-after has content, owner and edit flag" bash -c "grep -q '^--- owner uid:gid=' '$R/s6-a-after.txt' && grep -qE '^contains_edited_by_dsh=(yes|no)$' '$R/s6-a-after.txt'"
chk "format has both counts" grep -qE '^json_lines=[0-9]+ text_lines=[0-9]+$' "$R/s6-format.txt"
chk "scratch dir removed" test ! -e "$DIR/scratch"
if [ "${1:-}" != "--basic" ]; then
  for v in a b; do
    chk "kill-$v has leftover line" grep -qE '^leftover:' "$R/s6-kill-$v.txt"
    chk "kill-$v has sweep line" grep -qE '^sweep:' "$R/s6-kill-$v.txt"
    chk "kill-$v has final line" grep -qE '^final:' "$R/s6-kill-$v.txt"
  done
fi
exit $fail
