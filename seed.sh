#!/usr/bin/env bash
# =============================================================================
# Chat Systems — Seed Script
# =============================================================================
# Creates a workspace + test users with different roles for local development.
#
# Prerequisites: docker compose up -d (backend must be running)
#
# Usage:
#   ADMIN_EMAIL=admin@dev.local ADMIN_PASSWORD=... ./seed.sh
#
# Reads the admin credentials from the environment (same values you set in .env).
# All test users are created with the admin's password hash, so they share the
# admin password.
# =============================================================================

set -euo pipefail

API="http://localhost:3000/api"
ADMIN_EMAIL="${ADMIN_EMAIL:-admin@dev.local}"
ADMIN_PASSWORD="${ADMIN_PASSWORD:?ADMIN_PASSWORD must be set (the value from your .env)}"
PSQL="docker compose exec -T postgres psql -U chat -d chatsystems -t -A"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log()  { echo -e "${CYAN}[seed]${NC} $1"; }
ok()   { echo -e "${GREEN}[seed]${NC} $1"; }
err()  { echo -e "${RED}[seed]${NC} $1"; }

# ── 1. Login as admin ──────────────────────────────────────────────────────
log "Logging in as admin..."
LOGIN_PAYLOAD=$(python3 -c "import json,os; print(json.dumps({'email': os.environ['ADMIN_EMAIL'], 'password': os.environ['ADMIN_PASSWORD']}))")
LOGIN_RESP=$(curl -s -X POST "$API/auth/login" \
  -H "Content-Type: application/json" \
  -d "$LOGIN_PAYLOAD")

TOKEN=$(echo "$LOGIN_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('access_token',''))" 2>/dev/null || true)

if [ -z "$TOKEN" ]; then
  err "Login failed. Response: $LOGIN_RESP"
  err "Make sure backend is running: docker compose up -d"
  exit 1
fi

ADMIN_ID=$(echo "$LOGIN_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['user']['id'])" 2>/dev/null)
ok "Logged in as admin ($ADMIN_ID)"

# ── 2. Get admin's password hash (we'll reuse it for test users) ───────────
ADMIN_HASH=$(docker compose exec -T postgres psql -U chat -d chatsystems -t -A -q \
  -c "SELECT password_hash FROM users WHERE id = '$ADMIN_ID'" | head -1 | tr -d '\r\n')

if [ -z "$ADMIN_HASH" ]; then
  err "Could not get admin password hash from DB"
  exit 1
fi
log "Got admin password hash for reuse"

# ── 3. Create workspace ───────────────────────────────────────────────────
log "Creating workspace 'Dev Team'..."
WS_RESP=$(curl -s -X POST "$API/workspaces" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"name":"Dev Team","description":"Development workspace for testing roles and features"}')

WS_ID=$(echo "$WS_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('id',''))" 2>/dev/null || true)

if [ -z "$WS_ID" ]; then
  err "Failed to create workspace. Response: $WS_RESP"
  exit 1
fi
ok "Workspace created: Dev Team ($WS_ID)"

# Get the default #general channel ID
GENERAL_CH=$(docker compose exec -T postgres psql -U chat -d chatsystems -t -A -q \
  -c "SELECT id FROM channels WHERE workspace_id = '$WS_ID' AND is_default = true LIMIT 1" | head -1 | tr -d '\r\n')
log "Default channel #general: $GENERAL_CH"

# ── 4. Create test users ──────────────────────────────────────────────────
create_user() {
  local email="$1"
  local display_name="$2"
  local ws_role="$3"

  log "Creating ${BOLD}$display_name${NC} ($email) as ${BOLD}$ws_role${NC}..."

  # Insert user directly into DB (active, with admin's password hash)
  local user_id
  user_id=$(docker compose exec -T postgres psql -U chat -d chatsystems -t -A -q -c "
    INSERT INTO users (email, password_hash, display_name, status, is_instance_admin)
    VALUES ('$email', '$ADMIN_HASH', '$display_name', 'active', false)
    ON CONFLICT (email) DO UPDATE SET display_name = EXCLUDED.display_name
    RETURNING id;
  " | head -1 | tr -d '\r\n')

  # Add to workspace
  docker compose exec -T postgres psql -U chat -d chatsystems -q -c "
    INSERT INTO workspace_members (workspace_id, user_id, role)
    VALUES ('$WS_ID', '$user_id', '$ws_role')
    ON CONFLICT (workspace_id, user_id) DO NOTHING;
  " > /dev/null 2>&1

  # Add to #general channel
  if [ -n "$GENERAL_CH" ]; then
    docker compose exec -T postgres psql -U chat -d chatsystems -q -c "
      INSERT INTO channel_members (channel_id, user_id, role)
      VALUES ('$GENERAL_CH', '$user_id', 'member')
      ON CONFLICT (channel_id, user_id) DO NOTHING;
    " > /dev/null 2>&1
  fi

  ok "  → $display_name ($email) as $ws_role [$user_id]"
}

# Also add admin to workspace (already owner from create_workspace call)

create_user "alice@dev.local"   "Alice Johnson"   "admin"
create_user "bob@dev.local"     "Bob Smith"        "member"
create_user "charlie@dev.local" "Charlie Brown"    "member"
create_user "diana@dev.local"   "Diana Prince"     "guest"

# ── 5. Create an extra #random channel ─────────────────────────────────────
log "Creating #random channel..."
RANDOM_CH_RESP=$(curl -s -X POST "$API/workspaces/$WS_ID/channels" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"name":"random","description":"Off-topic and fun stuff"}')

RANDOM_CH=$(echo "$RANDOM_CH_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('id',''))" 2>/dev/null || true)
if [ -n "$RANDOM_CH" ]; then
  ok "  → #random ($RANDOM_CH)"
fi

# ── Done ───────────────────────────────────────────────────────────────────
echo ""
ok "══════════════════════════════════════════════════════════════"
ok "  ${BOLD}Seed complete!${NC}"
ok ""
ok "  Workspace:  Dev Team"
ok "  Channels:   #general, #random"
ok ""
ok "  ${BOLD}Test accounts${NC} (all share the admin password from \$ADMIN_PASSWORD):"
ok "  ┌────────────────────────┬──────────────────┬─────────┐"
ok "  │ Email                  │ Name             │ Role    │"
ok "  ├────────────────────────┼──────────────────┼─────────┤"
ok "  │ admin@dev.local        │ Admin            │ owner   │"
ok "  │ alice@dev.local        │ Alice Johnson    │ admin   │"
ok "  │ bob@dev.local          │ Bob Smith        │ member  │"
ok "  │ charlie@dev.local      │ Charlie Brown    │ member  │"
ok "  │ diana@dev.local        │ Diana Prince     │ guest   │"
ok "  └────────────────────────┴──────────────────┴─────────┘"
ok ""
ok "  Frontend: cd frontend && npm run dev"
ok "══════════════════════════════════════════════════════════════"
