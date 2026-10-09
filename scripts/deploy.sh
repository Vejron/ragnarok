#!/usr/bin/env bash
# Build locally, ship the image over SSH and swap the container on the VPS.
# Usage: DOMAIN=game.example.com scripts/deploy.sh user@host
# Optional: REMOTE_ACME_DIR, PLATFORM, HEALTH_TIMEOUT, CARGO_JOBS, SSH_OPTS, ACME_STAGING=1
set -euo pipefail

HOST="${1:-${DEPLOY_HOST:-}}"
DOMAIN="${DOMAIN:-}"
REMOTE_ACME_DIR="${REMOTE_ACME_DIR:-/var/lib/ragnarok/acme}"
PLATFORM="${PLATFORM:-linux/amd64}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-120}"
CARGO_JOBS="${CARGO_JOBS:-4}"
SSH_OPTS="${SSH_OPTS:-}"
NAME=ragnarok
IMAGE="$NAME:$(git rev-parse --short HEAD)"
STAMP="$(date +%Y%m%d%H%M%S)"

[[ -n "$HOST" ]] || { echo "usage: DOMAIN=... $0 user@host" >&2; exit 1; }
[[ "$DOMAIN" =~ ^[A-Za-z0-9.-]+$ ]] || { echo "set DOMAIN to a valid hostname" >&2; exit 1; }
[[ "$HEALTH_TIMEOUT" =~ ^[0-9]+$ && "$CARGO_JOBS" =~ ^[0-9]+$ ]] || { echo "bad numeric option" >&2; exit 1; }
for tool in npm docker ssh gzip; do command -v "$tool" >/dev/null || { echo "missing $tool" >&2; exit 1; }; done

# shellcheck disable=SC2086
remote() { ssh $SSH_OPTS "$HOST" "$@"; }

echo "==> remote preflight"
remote bash -s -- "$REMOTE_ACME_DIR" <<'PREFLIGHT'
set -euo pipefail
docker info >/dev/null
sudo=""; [ "$(id -u)" = 0 ] || sudo="sudo"
$sudo mkdir -p "$1" && $sudo chown 10001:10001 "$1"
PREFLIGHT

echo "==> building client"
(cd client && npm ci && npm run build)

echo "==> building image $IMAGE ($PLATFORM)"
docker buildx build --platform "$PLATFORM" --load --build-arg "CARGO_BUILD_JOBS=$CARGO_JOBS" -t "$IMAGE" -t "$NAME:latest" .

echo "==> uploading image"
docker save "$IMAGE" | gzip | remote "gunzip | docker load"

echo "==> swapping container"
remote bash -s -- "$NAME" "$IMAGE" "$DOMAIN" "$REMOTE_ACME_DIR" "$STAMP" "$HEALTH_TIMEOUT" "${ACME_STAGING:-0}" <<'REMOTE'
set -euo pipefail
name=$1 image=$2 domain=$3 acme=$4 stamp=$5 timeout=$6 staging=$7
old="$name-prev-$stamp"
had_old=0

if docker inspect "$name" >/dev/null 2>&1; then
  case "$(docker inspect -f '{{.Config.Image}}' "$name")" in
    "$name":*) ;;
    *) echo "container '$name' is not a ragnarok image; refusing to replace it" >&2; exit 1 ;;
  esac
  docker rename "$name" "$old"; docker stop "$old" >/dev/null; had_old=1
fi

rollback() {
  echo "deploy failed; rolling back" >&2
  docker logs --tail 40 "$name" >&2 || true
  docker rm -f "$name" >/dev/null 2>&1 || true
  if [ "$had_old" = 1 ]; then docker rename "$old" "$name"; docker start "$name" >/dev/null; fi
  exit 1
}
trap rollback ERR

env_args=(-e "RAGNAROK_DOMAIN=$domain")
[ "$staging" != 0 ] && env_args+=(-e RAGNAROK_ACME_STAGING=1)
docker run -d --name "$name" --restart unless-stopped \
  --sysctl net.ipv4.ip_unprivileged_port_start=0 \
  -p 80:80/tcp -p 443:443/tcp -p 443:443/udp \
  -v "$acme:/acme" "${env_args[@]}" \
  --label "ragnarok.release=$stamp" "$image" >/dev/null

# First start issues the certificate and exits; --restart brings it back with the cert loaded.
deadline=$((SECONDS + timeout))
until curl -fsS -o /dev/null --max-time 5 --resolve "$domain:443:127.0.0.1" $([ "$staging" != 0 ] && echo -k) "https://$domain/"; do
  [ "$SECONDS" -lt "$deadline" ] || false
  sleep 3
done
trap - ERR
echo "healthy: https://$domain/"

docker ps -a --format '{{.Names}}' | grep "^$name-prev-" | sort | head -n -1 | xargs -r docker rm >/dev/null || true
REMOTE
echo "==> done: https://$DOMAIN/"
