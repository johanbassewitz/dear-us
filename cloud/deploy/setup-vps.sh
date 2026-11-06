#!/usr/bin/env bash
# One-time setup of the Dear Us cloud on a fresh Ubuntu 24.04 VPS (e.g. Hetzner CX22).
# Run as root from the cloud/ folder copied to the server:
#   scp -r cloud root@<server-ip>:/opt/dearus     (or git clone the repository there)
#   ssh root@<server-ip> 'cd /opt/dearus && bash deploy/setup-vps.sh'
# Safe to run again: it only adds what's missing.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== Dear Us cloud setup"
if [ "$(id -u)" -ne 0 ]; then echo "Run as root (sudo bash deploy/setup-vps.sh)"; exit 1; fi

# 1) Docker + firewall
if ! command -v docker >/dev/null; then
  apt-get update -q
  apt-get install -y -q docker.io docker-compose-v2 ufw cron
  systemctl enable --now docker
fi
ufw allow OpenSSH >/dev/null; ufw allow 80/tcp >/dev/null; ufw allow 443/tcp >/dev/null
ufw --force enable >/dev/null

# 2) .env with generated secrets (you fill in the service keys)
if [ ! -f .env ]; then
  cp .env.example .env
  sed -i "s|^SERVER_SECRET=.*|SERVER_SECRET=$(openssl rand -hex 32)|" .env
  sed -i "s|^POSTGRES_PASSWORD=.*|POSTGRES_PASSWORD=$(openssl rand -hex 24)|" .env
  read -rp "Your API domain (e.g. dearus-api.axiantlabs.dev): " domain
  sed -i "s|^API_DOMAIN=.*|API_DOMAIN=${domain}|; s|^PUBLIC_URL=.*|PUBLIC_URL=https://${domain}|" .env
  chmod 600 .env
  echo
  echo "Created .env with fresh secrets. Now add your R2, Gemini, Resend and Microsoft keys:"
  echo "  nano $(pwd)/.env"
  echo "then run this script again."
  exit 0
fi
chmod 600 .env

# 3) daily database backup (14 days kept); the mailbox is temporary, accounts and purchases aren't
mkdir -p /var/backups/dearus
cat > /etc/cron.daily/dearus-db-backup <<CRON
#!/bin/sh
cd $(pwd) && docker compose exec -T db pg_dump -U dearus dearus | gzip > /var/backups/dearus/dearus-\$(date +%F).sql.gz
find /var/backups/dearus -name 'dearus-*.sql.gz' -mtime +14 -delete
CRON
chmod 755 /etc/cron.daily/dearus-db-backup

# 4) start (or update) everything
docker compose up -d --build
domain=$(grep '^API_DOMAIN=' .env | cut -d= -f2)
echo
echo "== Started. Checking https://${domain}/v1/health (the certificate can take a minute) ..."
for i in $(seq 1 30); do
  if curl -fsS "https://${domain}/v1/health" 2>/dev/null; then echo; echo "== Dear Us cloud is up ♡"; exit 0; fi
  sleep 5
done
echo "Not reachable yet. Check that the domain's DNS (A record) points to this server, then: docker compose logs -f"
