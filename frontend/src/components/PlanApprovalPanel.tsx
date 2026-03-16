"use client";

import { useState } from "react";
import { approvePlan, rejectPlan } from "@/lib/api";
import { Markdown } from "@/components/Markdown";

export function PlanApprovalPanel({
  taskId,
  plan,
  onAction,
}: {
  taskId: string;
  plan: string;
  onAction: () => void;
}) {
  const [showFeedback, setShowFeedback] = useState(false);
  const [feedback, setFeedback] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleApprove = async () => {
    setLoading(true);
    setError(null);
    try {
      await approvePlan(taskId);
      onAction();
    } catch (e) {
      setError(e instanceof Error ? e.message : "承認に失敗しました");
    } finally {
      setLoading(false);
    }
  };

  const handleReplan = async () => {
    setLoading(true);
    setError(null);
    try {
      await rejectPlan(taskId, "replan", feedback || undefined);
      setShowFeedback(false);
      setFeedback("");
      onAction();
    } catch (e) {
      setError(e instanceof Error ? e.message : "再計画に失敗しました");
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = async () => {
    setLoading(true);
    setError(null);
    try {
      await rejectPlan(taskId, "cancel");
      onAction();
    } catch (e) {
      setError(e instanceof Error ? e.message : "キャンセルに失敗しました");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* 計画表示 */}
      <div>
        <h3 className="text-sm font-semibold text-gh-text-secondary mb-2">実装計画</h3>
        <div className="p-3 bg-gh-surface border border-gh-border rounded-lg overflow-auto max-h-[500px]">
          <Markdown>{plan}</Markdown>
        </div>
      </div>

      {error && <div className="text-gh-red text-sm">{error}</div>}

      {/* アクションボタン */}
      <div className="flex items-start gap-2">
        <button
          onClick={handleApprove}
          disabled={loading}
          className="px-4 py-2 bg-gh-green/90 text-white rounded-md hover:bg-gh-green transition text-sm font-medium disabled:opacity-50"
        >
          承認して実行
        </button>
        <button
          onClick={() => setShowFeedback(!showFeedback)}
          disabled={loading}
          className="px-4 py-2 bg-gh-orange/15 text-gh-orange rounded-md hover:bg-gh-orange/25 transition text-sm font-medium disabled:opacity-50"
        >
          フィードバック付き再計画
        </button>
        <button
          onClick={handleCancel}
          disabled={loading}
          className="px-4 py-2 border border-gh-red/40 text-gh-red rounded-md hover:bg-gh-red/10 transition text-sm font-medium disabled:opacity-50"
        >
          キャンセル
        </button>
      </div>

      {/* フィードバック入力 */}
      {showFeedback && (
        <div className="p-3 bg-gh-surface border border-gh-orange/30 rounded-lg">
          <textarea
            value={feedback}
            onChange={(e) => setFeedback(e.target.value)}
            rows={3}
            className="w-full px-3 py-2 bg-gh-canvas border border-gh-border rounded-md text-sm text-gh-text placeholder:text-gh-text-muted focus:outline-none focus:border-gh-orange focus:ring-1 focus:ring-gh-orange/40 resize-none"
            placeholder="計画へのフィードバックを入力（例: テスト方針を変更してほしい、○○の方法で実装してほしい）"
          />
          <button
            onClick={handleReplan}
            disabled={loading}
            className="mt-2 px-3 py-1.5 bg-gh-orange/90 text-white rounded-md hover:bg-gh-orange transition text-sm font-medium disabled:opacity-50"
          >
            {loading ? "送信中..." : "再計画を依頼"}
          </button>
        </div>
      )}
    </div>
  );
}
