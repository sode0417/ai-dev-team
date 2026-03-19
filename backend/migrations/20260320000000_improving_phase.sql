-- improving フェーズ: スプリント改善結果カラム追加
ALTER TABLE sprints ADD COLUMN improvement_results JSONB;
