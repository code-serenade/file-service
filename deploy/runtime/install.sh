#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

IMAGE_TAR="${IMAGE_TAR:-file-service-images.tar}"
CONFIG_FILE="${CONFIG_FILE:-deploy.env}"

fail() {
  echo "error: $*" >&2
  exit 1
}

escape_toml_value() {
  printf '%s' "$1" | sed 's/[\\&|"]/\\&/g'
}

if [ "${ALLOW_NON_LINUX:-0}" != "1" ]; then
  case "$(uname -m)" in
    x86_64|amd64) ;;
    *) fail "this bundle requires Linux x86_64; detected $(uname -m)" ;;
  esac
  [ "$(uname -s)" = "Linux" ] || fail "this installer only supports Linux"
fi
command -v docker >/dev/null 2>&1 || fail "Docker is not installed"
docker info >/dev/null 2>&1 || fail "Docker daemon is unavailable or access is denied"
docker compose version >/dev/null 2>&1 || fail "Docker Compose plugin is not installed"
[ -f "$IMAGE_TAR" ] || fail "missing $IMAGE_TAR"
[ -f "$CONFIG_FILE" ] || fail "missing deployment configuration: $CONFIG_FILE"

set -a
. "$CONFIG_FILE"
set +a

for variable_name in \
  FILE_PORT AUTH_JWT_VERIFY_URL AUTH_AVATAR_PROVIDER_URL AUTH_INTERNAL_HEADER AUTH_INTERNAL_TOKEN USER_KEY_SALT \
  S3_PROVIDER S3_ENDPOINT S3_INTERNAL_ENDPOINT S3_ACCESS_KEY S3_SECRET_KEY S3_BUCKET_IDS
do
  eval "value=\${$variable_name:-}"
  [ -n "$value" ] || fail "$variable_name is required in deploy.env"
  case "$value" in
    replace_with_*) fail "configure $variable_name in deploy.env before installation" ;;
  esac
done

mkdir -p config logs

auth_jwt_verify_url="$(escape_toml_value "$AUTH_JWT_VERIFY_URL")"
auth_avatar_provider_url="$(escape_toml_value "${AUTH_AVATAR_PROVIDER_URL:-}")"
auth_internal_header="$(escape_toml_value "$AUTH_INTERNAL_HEADER")"
auth_internal_token="$(escape_toml_value "$AUTH_INTERNAL_TOKEN")"
user_key_salt="$(escape_toml_value "$USER_KEY_SALT")"
s3_provider="$(escape_toml_value "$S3_PROVIDER")"
s3_addressing_style="$(escape_toml_value "${S3_ADDRESSING_STYLE:-path}")"
s3_endpoint="$(escape_toml_value "$S3_ENDPOINT")"
s3_internal_endpoint="$(escape_toml_value "$S3_INTERNAL_ENDPOINT")"
s3_region="$(escape_toml_value "${S3_REGION:-}")"
s3_access_key="$(escape_toml_value "$S3_ACCESS_KEY")"
s3_secret_key="$(escape_toml_value "$S3_SECRET_KEY")"

{
  printf '%s\n' '[http]' 'port = 19878' '' '[jwt_verify]'
  printf 'url = "%s"\navatar_provider_url = "%s"\nheader = "%s"\ntoken = "%s"\n\n' "$auth_jwt_verify_url" "$auth_avatar_provider_url" "$auth_internal_header" "$auth_internal_token"
  printf '[storage]\nuser_key_salt = "%s"\n\n' "$user_key_salt"
  printf '[s3]\nprovider = "%s"\naddressing_style = "%s"\nendpoint = "%s"\ninternal_endpoint = "%s"\n' "$s3_provider" "$s3_addressing_style" "$s3_endpoint" "$s3_internal_endpoint"
  [ -z "$s3_region" ] || printf 'region = "%s"\n' "$s3_region"
  printf 'access_key = "%s"\nsecret_key = "%s"\n' "$s3_access_key" "$s3_secret_key"
  IFS=',' read -r -a bucket_ids <<< "$S3_BUCKET_IDS"
  for bucket_id in "${bucket_ids[@]}"; do
    bucket_id="${bucket_id//[[:space:]]/}"
    case "$bucket_id" in
      ''|*[!a-z0-9_]*) fail "S3 bucket IDs must use lowercase letters, digits, or underscores" ;;
    esac
    bucket_suffix="$(printf '%s' "$bucket_id" | tr '[:lower:]' '[:upper:]')"
    bucket_var="S3_BUCKET_${bucket_suffix}"
    policy_var="S3_BUCKET_${bucket_suffix}_READ_POLICY"
    prefix_var="S3_BUCKET_${bucket_suffix}_KEY_PREFIX"
    bucket_name="${!bucket_var:-}"
    read_policy="${!policy_var:-private}"
    key_prefix="${!prefix_var:-}"
    [ -n "$bucket_name" ] || fail "$bucket_var is required in deploy.env"
    case "$read_policy" in
      public|private) ;;
      *) fail "$policy_var must be public or private" ;;
    esac
    printf '\n[[s3.buckets]]\nid = "%s"\nbucket = "%s"\nread_policy = "%s"\nkey_prefix = "%s"\n' \
      "$(escape_toml_value "$bucket_id")" \
      "$(escape_toml_value "$bucket_name")" \
      "$read_policy" \
      "$(escape_toml_value "$key_prefix")"
  done
} > config/services.toml
chmod 600 config/services.toml

echo "==> Loading Linux x86_64 Docker image"
docker load -i "$IMAGE_TAR"

echo "==> Starting file service"
docker compose --env-file "$CONFIG_FILE" up -d file

echo "File service: http://localhost:$FILE_PORT"
