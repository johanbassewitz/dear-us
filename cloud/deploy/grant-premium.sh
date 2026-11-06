#!/usr/bin/env bash
# Gives the shared space(s) of an account Premium for N days (e.g. Microsoft's certification
# testers, or a couple you want to thank). Run on the server from the cloud/ folder:
#   bash deploy/grant-premium.sh tester@example.com 30
set -euo pipefail
cd "$(dirname "$0")/.."
email="${1:?usage: grant-premium.sh <email> [days]}"
days="${2:-30}"
[[ "$days" =~ ^[0-9]+$ ]] || { echo "days must be a number"; exit 1; }
docker compose exec -T db psql -U dearus dearus -v ON_ERROR_STOP=1 -v email="$email" -v days="$days" <<'SQL'
update spaces s set premium_until = greatest(coalesce(s.premium_until, now()), now()) + make_interval(days => :'days'::int)
  from members m join users u on u.id = m.user_id
 where m.space_id = s.id and u.email = lower(:'email')
returning s.id as space, s.premium_until;
SQL
