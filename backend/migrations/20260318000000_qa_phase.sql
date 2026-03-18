-- QA Agent フェーズ: Playwright MCP によるフロントエンドテスト
ALTER TABLE execution_sessions
    ADD COLUMN qa_output TEXT,
    ADD COLUMN qa_passed BOOLEAN,
    ADD COLUMN qa_screenshots JSONB;
