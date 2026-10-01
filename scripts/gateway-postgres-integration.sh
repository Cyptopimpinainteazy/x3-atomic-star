#!/usr/bin/env bash
# Real Postgres integration gate for x3-gateway.
#
# Owns a disposable Postgres container, waits for readiness, runs the ignored
# Rust integration test against the production Database::connect() path, and
# always removes the container on exit.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

IMAGE="${X3_GATEWAY_POSTGRES_IMAGE:-postgres:16-alpine}"
NAME="x3-gateway-postgres-$PPID-$$"
DB_USER="x3"
DB_PASSWORD="x3-test-only"
DB_NAME="x3_gateway_test"

if ! command -v docker >/dev/null 2>&1; then
  echo "gateway-postgres-integration: docker is required; no Postgres test container was started" >&2
  exit 2
fi

if ! docker info >/dev/null 2>&1; then
  echo "gateway-postgres-integration: docker daemon is not reachable" >&2
  exit 2
fi

cleanup() {
  status=$?
  trap - EXIT
  if [ "$status" -ne 0 ]; then
    echo "gateway-postgres-integration: Postgres logs follow because the gate failed" >&2
    docker logs "$NAME" >&2 2>/dev/null || true
  fi
  docker rm -f "$NAME" >/dev/null 2>&1 || true
  exit "$status"
}
trap cleanup EXIT

echo "gateway-postgres-integration: starting $IMAGE as $NAME"
docker run --detach --rm \
  --name "$NAME" \
  -e "POSTGRES_USER=$DB_USER" \
  -e "POSTGRES_PASSWORD=$DB_PASSWORD" \
  -e "POSTGRES_DB=$DB_NAME" \
  -P \
  "$IMAGE" >/dev/null

ready=0
for _ in $(seq 1 60); do
  if docker exec "$NAME" pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 1
done

if [ "$ready" -ne 1 ]; then
  echo "gateway-postgres-integration: Postgres did not become ready within 60 seconds" >&2
  exit 1
fi

HOST_PORT="$(docker inspect --format '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$NAME")"
if [ -z "$HOST_PORT" ]; then
  echo "gateway-postgres-integration: Docker did not publish Postgres port 5432" >&2
  exit 1
fi

export X3_GATEWAY_TEST_DATABASE_URL="postgres://$DB_USER:$DB_PASSWORD@127.0.0.1:$HOST_PORT/$DB_NAME"

echo "gateway-postgres-integration: running x3-gateway database integration test on 127.0.0.1:$HOST_PORT"
env SKIP_WASM_BUILD=1 \
  cargo test --locked -p x3-gateway --test postgres_integration -- \
  --ignored --nocapture --test-threads=1
