#!/usr/bin/env bash
# 汎用デプロイスクリプト
# deploy.json を読み取り、サービスのビルド・マイグレーション・再起動を行う
#
# 使い方:
#   ./scripts/deploy.sh          # 全サービスをデプロイ
#   ./scripts/deploy.sh backend  # 指定サービスのみ
#
# 前提:
#   - deploy.json がリポジトリルートに存在
#   - jq がインストール済み
#   - .env がバックエンドディレクトリに存在（DB接続等）

set -euo pipefail

REPO_ROOT="${DEPLOY_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
DEPLOY_CONFIG="$REPO_ROOT/deploy.json"
TARGET_SERVICE="${1:-}"

log() { echo "[deploy] $(date '+%H:%M:%S') $*"; }
err() { echo "[deploy] ERROR: $*" >&2; }

if [[ ! -f "$DEPLOY_CONFIG" ]]; then
  err "deploy.json が見つかりません: $DEPLOY_CONFIG"
  exit 1
fi

if ! command -v jq &>/dev/null; then
  err "jq が必要です: brew install jq"
  exit 1
fi

# マイグレーション適用（未適用分のみ）
apply_migrations() {
  local migrations_dir="$1"
  local full_path="$REPO_ROOT/$migrations_dir"

  if [[ ! -d "$full_path" ]]; then
    log "マイグレーションディレクトリなし: $migrations_dir"
    return 0
  fi

  # .env から DATABASE_URL を読み取り
  local db_url=""
  for env_file in "$REPO_ROOT/.env" "$REPO_ROOT/backend/.env"; do
    if [[ -f "$env_file" ]]; then
      db_url=$(grep '^DATABASE_URL=' "$env_file" | head -1 | cut -d= -f2-)
      break
    fi
  done

  if [[ -z "$db_url" ]]; then
    err "DATABASE_URL が見つかりません"
    return 1
  fi

  # migration_history テーブルを作成（なければ）
  psql "$db_url" -c "
    CREATE TABLE IF NOT EXISTS migration_history (
      filename TEXT PRIMARY KEY,
      applied_at TIMESTAMPTZ DEFAULT NOW()
    );
  " &>/dev/null

  local applied=0
  for sql_file in "$full_path"/*.sql; do
    [[ ! -f "$sql_file" ]] && continue
    local filename
    filename=$(basename "$sql_file")

    # 適用済みかチェック
    local exists
    exists=$(psql "$db_url" -tAc "SELECT 1 FROM migration_history WHERE filename = '$filename';" 2>/dev/null || echo "")
    if [[ "$exists" == "1" ]]; then
      continue
    fi

    log "マイグレーション適用: $filename"
    if psql "$db_url" -f "$sql_file" &>/dev/null; then
      psql "$db_url" -c "INSERT INTO migration_history (filename) VALUES ('$filename');" &>/dev/null
      applied=$((applied + 1))
    else
      err "マイグレーション失敗: $filename"
      return 1
    fi
  done

  if [[ $applied -gt 0 ]]; then
    log "マイグレーション $applied 件適用完了"
  else
    log "新規マイグレーションなし"
  fi
}

# プロセス停止
stop_service() {
  local port="$1"
  local pids
  pids=$(lsof -ti ":$port" 2>/dev/null || true)
  if [[ -n "$pids" ]]; then
    log "ポート $port のプロセスを停止: $pids"
    echo "$pids" | xargs kill 2>/dev/null || true
    sleep 2
    # まだ残っていたら強制終了
    pids=$(lsof -ti ":$port" 2>/dev/null || true)
    if [[ -n "$pids" ]]; then
      echo "$pids" | xargs kill -9 2>/dev/null || true
      sleep 1
    fi
  fi
}

# Rust サービスのデプロイ
deploy_rust() {
  local name="$1" dir="$2" port="$3" build_cmd="$4" binary="$5" log_file="$6" error_log="$7"

  log "[$name] ビルド開始"
  local service_dir="$REPO_ROOT/$dir"
  (cd "$service_dir" && eval "$build_cmd") || { err "[$name] ビルド失敗"; return 1; }
  log "[$name] ビルド完了"

  stop_service "$port"

  # expand ~ in log paths
  log_file="${log_file/#\~/$HOME}"
  error_log="${error_log/#\~/$HOME}"
  mkdir -p "$(dirname "$log_file")"

  log "[$name] 起動: ポート $port"
  (cd "$REPO_ROOT/$dir" && nohup "$REPO_ROOT/$dir/$binary" >> "$log_file" 2>> "$error_log" &)
  sleep 2

  if lsof -ti ":$port" &>/dev/null; then
    log "[$name] 起動成功"
  else
    err "[$name] 起動失敗 — ログを確認: $error_log"
    return 1
  fi
}

# Next.js サービスのデプロイ
deploy_nextjs() {
  local name="$1" dir="$2" port="$3" build_cmd="$4" start_cmd="$5"

  local service_dir="$REPO_ROOT/$dir"

  log "[$name] ビルド開始"
  (cd "$service_dir" && eval "$build_cmd") || { err "[$name] ビルド失敗"; return 1; }
  log "[$name] ビルド完了"

  stop_service "$port"

  log "[$name] 起動: ポート $port"
  (cd "$service_dir" && nohup $start_cmd > /dev/null 2>&1 &)
  sleep 3

  if lsof -ti ":$port" &>/dev/null; then
    log "[$name] 起動成功"
  else
    err "[$name] 起動失敗"
    return 1
  fi
}

# メイン処理
log "デプロイ開始: $(basename "$REPO_ROOT")"

# git pull
log "最新コードを取得"
(cd "$REPO_ROOT" && git pull origin main --ff-only) || { err "git pull 失敗"; exit 1; }

# サービスをループ
service_count=$(jq '.services | length' "$DEPLOY_CONFIG")

for i in $(seq 0 $((service_count - 1))); do
  name=$(jq -r ".services[$i].name" "$DEPLOY_CONFIG")
  type=$(jq -r ".services[$i].type" "$DEPLOY_CONFIG")

  # 特定サービス指定時はスキップ
  if [[ -n "$TARGET_SERVICE" && "$name" != "$TARGET_SERVICE" ]]; then
    continue
  fi

  # マイグレーション
  migrations_dir=$(jq -r ".services[$i].migrations_dir // empty" "$DEPLOY_CONFIG")
  if [[ -n "$migrations_dir" ]]; then
    apply_migrations "$migrations_dir"
  fi

  # タイプ別デプロイ
  dir=$(jq -r ".services[$i].dir" "$DEPLOY_CONFIG")
  port=$(jq -r ".services[$i].port" "$DEPLOY_CONFIG")
  build_cmd=$(jq -r ".services[$i].build_cmd" "$DEPLOY_CONFIG")

  case "$type" in
    rust)
      binary=$(jq -r ".services[$i].binary" "$DEPLOY_CONFIG")
      log_file=$(jq -r ".services[$i].log_file // \"/tmp/${name}.log\"" "$DEPLOY_CONFIG")
      error_log=$(jq -r ".services[$i].error_log // \"/tmp/${name}.error.log\"" "$DEPLOY_CONFIG")
      deploy_rust "$name" "$dir" "$port" "$build_cmd" "$binary" "$log_file" "$error_log"
      ;;
    nextjs)
      start_cmd=$(jq -r ".services[$i].start_cmd" "$DEPLOY_CONFIG")
      deploy_nextjs "$name" "$dir" "$port" "$build_cmd" "$start_cmd"
      ;;
    *)
      err "未対応のタイプ: $type"
      ;;
  esac
done

log "デプロイ完了"
