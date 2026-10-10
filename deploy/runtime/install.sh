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
  FILE_PORT AUTH_JWT_VERIFY_URL AUTH_INTERNAL_HEADER AUTH_INTERNAL_TOKEN USER_KEY_SALT \
  S3_BUCKET_IDS
do
  eval "value=\${$variable_name:-}"
  [ -n "$value" ] || fail "$variable_name is required in deploy.env"
  case "$value" in
    replace_with_*) fail "configure $variable_name in deploy.env before installation" ;;
  esac
done

mkdir -p config logs

shared_keys=0
if [ -n "${S3_ACCESS_KEY:-}" ] || [ -n "${S3_SECRET_KEY:-}" ]; then
  [ -n "${S3_ACCESS_KEY:-}" ] || fail "S3_ACCESS_KEY is required when using shared keys"
  [ -n "${S3_SECRET_KEY:-}" ] || fail "S3_SECRET_KEY is required when using shared keys"
  case "$S3_ACCESS_KEY$S3_SECRET_KEY" in
    *replace_with_*) fail "configure shared S3 credentials in deploy.env" ;;
  esac
  printf 'access_key = "%s"\nsecret_key = "%s"\n' \
    "$(escape_toml_value "$S3_ACCESS_KEY")" \
    "$(escape_toml_value "$S3_SECRET_KEY")" > config/keys.toml
  chmod 600 config/keys.toml
  shared_keys=1
else
  rm -f config/keys.toml
fi

auth_jwt_verify_url="$(escape_toml_value "$AUTH_JWT_VERIFY_URL")"
auth_internal_header="$(escape_toml_value "$AUTH_INTERNAL_HEADER")"
auth_internal_token="$(escape_toml_value "$AUTH_INTERNAL_TOKEN")"
user_key_salt="$(escape_toml_value "$USER_KEY_SALT")"
s3_provider="$(escape_toml_value "${S3_PROVIDER:-}")"
s3_addressing_style="$(escape_toml_value "${S3_ADDRESSING_STYLE:-}")"
s3_endpoint="$(escape_toml_value "${S3_ENDPOINT:-}")"
s3_internal_endpoint="$(escape_toml_value "${S3_INTERNAL_ENDPOINT:-}")"
s3_region="$(escape_toml_value "${S3_REGION:-}")"

{
  printf '%s\n' '[http]' 'port = 19878' '' '[jwt_verify]'
  printf 'url = "%s"\nheader = "%s"\ntoken = "%s"\n\n' "$auth_jwt_verify_url" "$auth_internal_header" "$auth_internal_token"
  printf '[storage]\nuser_key_salt = "%s"\n\n' "$user_key_salt"
  printf '[s3]\n'
  [ -z "$s3_provider" ] || printf 'provider = "%s"\n' "$s3_provider"
  [ -z "$s3_addressing_style" ] || printf 'addressing_style = "%s"\n' "$s3_addressing_style"
  [ -z "$s3_endpoint" ] || printf 'endpoint = "%s"\n' "$s3_endpoint"
  [ -z "$s3_internal_endpoint" ] || printf 'internal_endpoint = "%s"\n' "$s3_internal_endpoint"
  [ -z "$s3_region" ] || printf 'region = "%s"\n' "$s3_region"
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
    provider_var="S3_BUCKET_${bucket_suffix}_PROVIDER"
    endpoint_var="S3_BUCKET_${bucket_suffix}_ENDPOINT"
    internal_endpoint_var="S3_BUCKET_${bucket_suffix}_INTERNAL_ENDPOINT"
    addressing_style_var="S3_BUCKET_${bucket_suffix}_ADDRESSING_STYLE"
    region_var="S3_BUCKET_${bucket_suffix}_REGION"
    access_key_var="S3_BUCKET_${bucket_suffix}_ACCESS_KEY"
    secret_key_var="S3_BUCKET_${bucket_suffix}_SECRET_KEY"
    bucket_name="${!bucket_var:-}"
    read_policy="${!policy_var:-private}"
    key_prefix="${!prefix_var:-}"
    bucket_provider="$(escape_toml_value "${!provider_var:-}")"
    bucket_endpoint="$(escape_toml_value "${!endpoint_var:-}")"
    bucket_internal_endpoint="$(escape_toml_value "${!internal_endpoint_var:-}")"
    bucket_addressing_style="$(escape_toml_value "${!addressing_style_var:-}")"
    bucket_region="$(escape_toml_value "${!region_var:-}")"
    bucket_access_key="$(escape_toml_value "${!access_key_var:-}")"
    bucket_secret_key="$(escape_toml_value "${!secret_key_var:-}")"
    [ -n "$bucket_name" ] || fail "$bucket_var is required in deploy.env"
    [ -n "$bucket_endpoint" ] || [ -n "$s3_endpoint" ] || fail "$endpoint_var or S3_ENDPOINT is required in deploy.env"
    if [ "$shared_keys" = "0" ]; then
      [ -n "$bucket_access_key" ] || fail "$access_key_var is required when config/keys.toml is absent"
      [ -n "$bucket_secret_key" ] || fail "$secret_key_var is required when config/keys.toml is absent"
      case "$bucket_access_key$bucket_secret_key" in
        *replace_with_*) fail "configure credentials for $bucket_id in deploy.env" ;;
      esac
    fi
    case "$bucket_name" in
      replace_with_*) fail "configure $bucket_var in deploy.env" ;;
    esac
    case "$read_policy" in
      public|private) ;;
      *) fail "$policy_var must be public or private" ;;
    esac
    printf '\n[[s3.buckets]]\nid = "%s"\nbucket = "%s"\nread_policy = "%s"\nkey_prefix = "%s"\n' \
      "$(escape_toml_value "$bucket_id")" \
      "$(escape_toml_value "$bucket_name")" \
      "$read_policy" \
      "$(escape_toml_value "$key_prefix")"
    [ -z "$bucket_provider" ] || printf 'provider = "%s"\n' "$bucket_provider"
    [ -z "$bucket_addressing_style" ] || printf 'addressing_style = "%s"\n' "$bucket_addressing_style"
    [ -z "$bucket_endpoint" ] || printf 'endpoint = "%s"\n' "$bucket_endpoint"
    [ -z "$bucket_internal_endpoint" ] || printf 'internal_endpoint = "%s"\n' "$bucket_internal_endpoint"
    [ -z "$bucket_region" ] || printf 'region = "%s"\n' "$bucket_region"
    if [ "$shared_keys" = "0" ]; then
      printf 'access_key = "%s"\nsecret_key = "%s"\n' "$bucket_access_key" "$bucket_secret_key"
    fi
  done
} > config/services.toml
chmod 600 config/services.toml

echo "==> Loading Linux x86_64 Docker image"
docker load -i "$IMAGE_TAR"

echo "==> Starting file service"
docker compose --env-file "$CONFIG_FILE" up -d file

echo "File service: http://localhost:$FILE_PORT"
