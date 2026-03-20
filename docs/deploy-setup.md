# 自動デプロイ セットアップガイド

Mac mini self-hosted runner による自動デプロイの設定手順。
PRがmainにマージされると、自動でビルド・マイグレーション・再起動が実行される。

## 前提

- Mac mini に self-hosted runner がインストール済み (`~/actions-runner`)
- runner ラベル: `self-hosted`, `deploy`

## 新しいリポジトリへの適用手順

### 1. Runner を追加登録

```bash
# 登録トークンを取得
gh api repos/sode0417/<REPO>/actions/runners/registration-token -X POST --jq '.token'

# runner を追加登録（既存の runner に追加）
cd ~/actions-runner
./config.sh --url https://github.com/sode0417/<REPO> --token <TOKEN> --name mac-mini --labels self-hosted,macOS,ARM64,deploy --unattended --replace
```

> 注意: 1つの runner は1リポにしか登録できない。
> 複数リポで使う場合は Organization runner にするか、runner を複数インストールする。

### 2. deploy.json を作成

リポジトリルートに `deploy.json` を配置:

```json
{
  "services": [
    {
      "name": "backend",
      "type": "rust",
      "dir": ".",
      "port": 8001,
      "build_cmd": "cargo build --release",
      "binary": "target/release/<BINARY_NAME>",
      "log_file": "~/Library/Logs/<PROJECT>-backend.log",
      "error_log": "~/Library/Logs/<PROJECT>-backend.error.log",
      "migrations_dir": "migrations"
    }
  ]
}
```

#### サービスタイプ

| type | 必須フィールド | 説明 |
|------|---------------|------|
| `rust` | `binary`, `log_file`, `error_log` | cargo build → バイナリ起動 |
| `nextjs` | `start_cmd` | npm run build → next start |

#### 共通フィールド

| フィールド | 説明 |
|-----------|------|
| `name` | サービス名 |
| `dir` | ソースディレクトリ（リポルートからの相対パス） |
| `port` | リッスンポート（停止・起動確認に使用） |
| `build_cmd` | ビルドコマンド |
| `migrations_dir` | マイグレーション SQL のディレクトリ（省略可） |

### 3. deploy.sh をコピー

```bash
cp ~/Projects/ai-dev-team/scripts/deploy.sh <REPO>/scripts/deploy.sh
chmod +x <REPO>/scripts/deploy.sh
```

### 4. CI ワークフローにデプロイジョブを追加

`.github/workflows/ci.yml` に追加:

```yaml
on:
  push:
    branches: [main]

jobs:
  # ... 既存の CI ジョブ ...

  deploy:
    name: Deploy
    needs: [<CI_JOB_NAMES>]
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    runs-on: [self-hosted, deploy]

    steps:
      - uses: actions/checkout@v4

      - name: Deploy
        run: ./scripts/deploy.sh
```

### 5. migration_history テーブルの初期化

既存のマイグレーションを履歴に登録:

```bash
psql $DATABASE_URL -c "
  CREATE TABLE IF NOT EXISTS migration_history (
    filename TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ DEFAULT NOW()
  );"

for f in migrations/*.sql; do
  psql $DATABASE_URL -c "INSERT INTO migration_history (filename) VALUES ('$(basename $f)') ON CONFLICT DO NOTHING;"
done
```
