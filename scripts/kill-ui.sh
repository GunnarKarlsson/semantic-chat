#!/usr/bin/env sh
set -eu

PIDS=$(lsof -t -iTCP:3000 -sTCP:LISTEN 2>/dev/null || true)

if [ -z "$PIDS" ]; then
  echo "nothing listening on port 3000"
  exit 0
fi

echo "killing: $PIDS"
kill $PIDS
echo "port 3000 is free"
