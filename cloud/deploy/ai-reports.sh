#!/usr/bin/env bash
# Lists reports about Cupid's answers that nobody has looked at yet, newest first.
#   bash deploy/ai-reports.sh          # list
#   bash deploy/ai-reports.sh done     # mark everything listed as handled
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "${1:-}" = "done" ]; then
  docker compose exec -T db psql -U dearus dearus -c "update ai_reports set handled_at = now() where handled_at is null;"
else
  docker compose exec -T db psql -U dearus dearus -c "select created_at, reason, left(coalesce(details, ''), 200) as details, left(coalesce(answer, ''), 400) as answer from ai_reports where handled_at is null order by created_at desc limit 50;"
fi
