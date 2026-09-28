#!/bin/sh
# Container entrypoint for systemprompt-internal.
# Authors a profile via `systemprompt admin setup` on first boot,
# waits for Postgres, runs migrations, starts the server.
set -eu

# One-click platforms (Railway et al.) export unfilled template variables as
# empty strings; admin setup would record "" as a configured provider key.
# Treat blank as unset.
[ -n "${ANTHROPIC_API_KEY:-}" ] || unset ANTHROPIC_API_KEY
[ -n "${OPENAI_API_KEY:-}" ] || unset OPENAI_API_KEY
[ -n "${GEMINI_API_KEY:-}" ] || unset GEMINI_API_KEY
[ -n "${GITHUB_TOKEN:-}" ] || unset GITHUB_TOKEN
[ -n "${EXTERNAL_URL:-}" ] || unset EXTERNAL_URL

# Platform-neutral external URL. Render injects RENDER_EXTERNAL_URL; every
# other catalog template sets EXTERNAL_URL explicitly.
EXTERNAL_URL="${EXTERNAL_URL:-${RENDER_EXTERNAL_URL:-}}"

PROFILE_DIR="${SYSTEMPROMPT_PROFILE_DIR:-/app/.systemprompt/profiles/docker}"
PROFILE_FILE="$PROFILE_DIR/profile.yaml"
SECRETS_FILE="$PROFILE_DIR/secrets.json"

if [ -n "${SYSTEMPROMPT_PROFILE_DIR:-}" ]; then
    # A profile directory was supplied (e.g. bind-mounted air-gap profile).
    # Do not generate anything — just validate the expected files exist.
    if [ ! -f "$PROFILE_FILE" ]; then
        echo "ERROR: SYSTEMPROMPT_PROFILE_DIR is set but $PROFILE_FILE is missing." >&2
        exit 1
    fi
    if [ ! -f "$SECRETS_FILE" ]; then
        echo "ERROR: SYSTEMPROMPT_PROFILE_DIR is set but $SECRETS_FILE is missing." >&2
        exit 1
    fi
    # A supplied profile names its own database; the readiness probe below
    # must not fall back to the compose-only `postgres` hostname.
    if [ -z "${DATABASE_URL:-}" ]; then
        DATABASE_URL="$(jq -r '.database_url // empty' "$SECRETS_FILE")"
        if [ -z "$DATABASE_URL" ]; then
            echo "ERROR: $SECRETS_FILE has no database_url." >&2
            exit 1
        fi
    fi
    # Why: the air-gap scenario mounts this directory read-only and may share
    # it between replicas, so nothing below mints into it. A key minted per
    # container would seal records the other replicas cannot open and sign
    # tokens they reject, and would be lost on every restart.
    if [ -z "$(jq -r '.encryption_master_key // empty' "$SECRETS_FILE")" ]; then
        echo "ERROR: $SECRETS_FILE has no encryption_master_key." >&2
        echo "  Core 0.62 refuses to boot without it. Add 64 hex characters" >&2
        echo "  (openssl rand -hex 32) to the shared secrets and redeploy." >&2
        exit 1
    fi
    # Multi-node deployments share one signing key through the
    # `signing_key_pem` secret; a single-node supplied profile may instead
    # point `security.signing_key_path` at a key file it ships.
    if [ -z "$(jq -r '.signing_key_pem // empty' "$SECRETS_FILE")" ]; then
        key_file="$(sed -n 's/^  signing_key_path:[[:space:]]*//p' "$PROFILE_FILE" | tr -d '"' | head -1)"
        key_file="${key_file:-/app/signing_key.pem}"
        case "$key_file" in /*) ;; *) key_file="$PROFILE_DIR/$key_file" ;; esac
        if [ ! -s "$key_file" ]; then
            echo "ERROR: $SECRETS_FILE has no signing_key_pem and $key_file does not exist." >&2
            echo "  Supply the shared signing key as signing_key_pem (base64 PEM)." >&2
            exit 1
        fi
    fi
    SIGNING_KEY_FROM_SECRETS=1
else
    if [ -z "${ANTHROPIC_API_KEY:-}" ] && [ -z "${OPENAI_API_KEY:-}" ] && [ -z "${GEMINI_API_KEY:-}" ]; then
        echo "ERROR: set at least one of ANTHROPIC_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY in .env" >&2
        exit 1
    fi
    if [ -z "${DATABASE_URL:-}" ]; then
        echo "ERROR: DATABASE_URL is required." >&2
        exit 1
    fi
    if [ ! -f "$PROFILE_FILE" ] && [ -z "${SYSTEMPROMPT_ADMIN_EMAIL:-}" ]; then
        echo "ERROR: SYSTEMPROMPT_ADMIN_EMAIL is required on first boot (the administrator admin setup creates)." >&2
        exit 1
    fi

    if [ ! -f "$PROFILE_FILE" ]; then
        echo "Generating profile via admin setup..."
        # Default provider = first configured key (setup picks up the
        # ANTHROPIC/OPENAI/GEMINI_API_KEY env vars itself).
        if [ -n "${ANTHROPIC_API_KEY:-}" ]; then DEFAULT_PROVIDER=anthropic
        elif [ -n "${OPENAI_API_KEY:-}" ]; then DEFAULT_PROVIDER=openai
        else DEFAULT_PROVIDER=gemini
        fi
        /app/bin/systemprompt admin setup -e docker \
            --default-provider "$DEFAULT_PROVIDER" --yes --no-migrate

        # Setup authors a localhost dev profile; patch the parts the
        # container environment dictates.
        # 1. Bind publicly (Render/compose port detection needs 0.0.0.0).
        #    Overridable via HOST for platforms whose internal networking is
        #    IPv6-only (Railway healthchecks need HOST=::).
        # Quoted: bare "::" (IPv6 any) is invalid YAML.
        sed -i "s/^  host: 127\.0\.0\.1$/  host: \"${HOST:-0.0.0.0}\"/" "$PROFILE_FILE"
        # 1b. Binaries ship in /app/bin, not a cargo target dir.
        sed -i 's|^  bin: .*|  bin: /app/bin|' "$PROFILE_FILE"
        # 2. Point at the real database, not setup's generated localhost one.
        jq --arg db "$DATABASE_URL" '.database_url = $db' "$SECRETS_FILE" \
            > "$SECRETS_FILE.tmp" && mv "$SECRETS_FILE.tmp" "$SECRETS_FILE"
        # 2b. Optional read/write split: reads stay on DATABASE_URL, writes go
        #     to the primary named here (core refuses a standby as write target).
        if [ -n "${DATABASE_WRITE_URL:-}" ]; then
            jq --arg db "$DATABASE_WRITE_URL" '.database_write_url = $db' "$SECRETS_FILE" \
                > "$SECRETS_FILE.tmp" && mv "$SECRETS_FILE.tmp" "$SECRETS_FILE"
        fi
        # 2c. The gateway accounting journal and at-rest sealing refuse to
        #     start without encryption_master_key, and setup does not mint it.
        if [ -z "$(jq -r '.encryption_master_key // empty' "$SECRETS_FILE")" ]; then
            key="$(od -An -tx1 -N32 /dev/urandom | tr -d ' \n')"
            jq --arg key "$key" '.encryption_master_key = $key' "$SECRETS_FILE" \
                > "$SECRETS_FILE.tmp" && mv "$SECRETS_FILE.tmp" "$SECRETS_FILE"
        fi
        chmod 600 "$SECRETS_FILE"
        # 3. Advertise the public URL when the platform provides one
        #    (EXTERNAL_URL, or RENDER_EXTERNAL_URL via the fallback above).
        if [ -n "${EXTERNAL_URL:-}" ]; then
            sed -i "s|^  api_external_url: .*|  api_external_url: ${EXTERNAL_URL}|" "$PROFILE_FILE"
            sed -i "/^  cors_allowed_origins:/a\\  - ${EXTERNAL_URL}" "$PROFILE_FILE"
        fi
    fi
fi

export SYSTEMPROMPT_PROFILE="$PROFILE_FILE"

# Probe DATABASE_URL directly when provided (managed Postgres, e.g. Render);
# fall back to the compose-style host/user/db vars otherwise.
if [ -n "${DATABASE_URL:-}" ]; then
    pg_probe() { pg_isready -d "$DATABASE_URL"; }
    echo "Waiting for Postgres at DATABASE_URL host..."
else
    PG_HOST="${PG_HOST:-postgres}"
    PG_USER="${PG_USER:-systemprompt}"
    PG_DB="${PG_DB:-systemprompt}"
    pg_probe() { pg_isready -h "$PG_HOST" -U "$PG_USER" -d "$PG_DB"; }
    echo "Waiting for Postgres at ${PG_HOST}..."
fi
i=0
until pg_probe >/dev/null 2>&1; do
    i=$((i + 1))
    if [ "$i" -ge 300 ]; then
        echo "ERROR: Postgres did not become ready within 300s." >&2
        exit 1
    fi
    sleep 1
done
echo "Postgres is ready."

if [ -z "${SIGNING_KEY_FROM_SECRETS:-}" ] && [ ! -f /app/signing_key.pem ]; then
    echo "Generating signing key..."
    /app/bin/systemprompt admin keys generate --output /app/signing_key.pem
fi

echo "Running database migrations..."
# A managed volume/database outlives the image, so a database seeded by an older
# tag can carry checksums for migrations that were since edited in the source
# tree. --repair-drift repairs that case, and only that case, then retries
# once; every other failure aborts boot with core's classified error and hint
# (a blind repair-and-retry used to repeat the same failure twice).
/app/bin/systemprompt infra db migrate --repair-drift

echo "Ensuring bootstrap admin user..."
/app/bin/systemprompt admin bootstrap

# web/dist is node-local and not shipped in the image. The scheduler's
# bootstrap run of publish_pipeline takes a database-wide advisory lock, so in
# a multi-node deployment only one node renders its site at boot and the rest
# serve 404 until a later tick lands on them. The manual runner takes no lock:
# render here, on every node, before the server accepts traffic. A failure is
# logged rather than fatal so a content problem never takes the gateway down.
echo "Publishing web assets for this node..."
if ! /app/bin/systemprompt infra jobs run publish_pipeline; then
    echo "WARN: publish_pipeline failed; the public site may 404 on this node until the next scheduled run." >&2
fi

echo "Starting services..."
exec /app/bin/systemprompt infra services start --foreground
