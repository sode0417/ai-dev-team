# Branch Protection + Auto-merge 設定手順

ai-dev-team が管理する全プロジェクトリポジトリに対して、main ブランチの保護と Auto-merge を設定する手順。

## 対象リポジトリ

| リポジトリ | CI 内容 |
|-----------|---------|
| sode0417/ai-dev-team | cargo build + cargo test + npm run build |
| sode0417/factrail | npm run build + npm run test |
| sode0417/f2a-backend | cargo build + cargo test |
| sode0417/f2a-frontend | npm run build |
| sode0417/ai-assistant | Python lint + test |
| sode0417/self-protocol | npm run build |
| sode0417/slack-cli | cargo build + cargo test |

## 手順 1: Auto-merge を有効化

各リポジトリで以下を実施:

1. GitHub リポジトリページ → **Settings** → **General**
2. **Pull Requests** セクションまでスクロール
3. **Allow auto-merge** にチェック → **Save**

## 手順 2: CI ワークフローを追加

各リポジトリに `.github/workflows/ci.yml` を追加する。

- **ai-dev-team**: 本リポジトリの `.github/workflows/ci.yml` をそのまま使用
- **その他のリポ**: `docs/ci-template.yml` をベースにカスタマイズ

## 手順 3: Branch Protection Rule を設定

各リポジトリで以下を実施:

1. GitHub リポジトリページ → **Settings** → **Branches**
2. **Add branch protection rule** をクリック
3. 以下の値を設定:

| 設定項目 | 値 |
|---------|---|
| Branch name pattern | `main` |
| Require a pull request before merging | OFF |
| Require status checks to pass before merging | ON |
| Require branches to be up to date before merging | OFF（推奨: コンフリクト監視が自動解消するため） |
| Status checks that are required | `Backend (Rust)`, `Frontend (Next.js)` ※リポによって異なる |
| Require conversation resolution before merging | OFF |
| Require signed commits | OFF |
| Require linear history | OFF |
| Include administrators | OFF（管理者は緊急マージ可能に） |
| Restrict who can push to matching branches | OFF |
| Allow force pushes | OFF |
| Allow deletions | OFF |

4. **Create** をクリック

### 必須ステータスチェック名の確認方法

Branch Protection の "Status checks that are required" に指定する名前は、CI ワークフローの `jobs.<job_id>.name` に対応する。

- `Backend (Rust)` — ai-dev-team, f2a-backend, slack-cli
- `Frontend (Next.js)` — ai-dev-team, f2a-frontend, factrail
- リポによって異なるため、初回 PR で CI が実行された後に設定するのが確実

## 手順 4: 動作確認

1. テスト用ブランチを作成し、PR を作成
2. CI が自動実行されることを確認
3. `gh pr merge --auto --squash --delete-branch` で Auto-merge を有効化
4. CI パス後に自動マージされることを確認

## 注意事項

- Branch Protection 未設定のリポでは `gh pr merge --auto` はエラーになるが、ai-dev-team の pipeline はこれを警告ログとして処理し、PR 作成自体は成功する
- Auto-merge はコンフリクトがあると停止する。ai-dev-team のコンフリクト監視ループが自動修復を試みる
- Required reviews は 0（不要）。bot が作成した PR を自動マージするため
