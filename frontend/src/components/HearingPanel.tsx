"use client";

import { useState } from "react";
import type { TaskHearing, HearingAnswer } from "@/types";
import { answerHearing } from "@/lib/api";

export function HearingPanel({
  taskId,
  hearings,
  onAnswered,
}: {
  taskId: string;
  hearings: TaskHearing[];
  onAnswered: () => void;
}) {
  const pendingHearing = hearings.find((h) => h.status === "pending");
  const answeredHearings = hearings.filter((h) => h.status === "answered");

  return (
    <div className="space-y-4">
      {/* 過去のヒアリング回答 */}
      {answeredHearings.length > 0 && (
        <div>
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase tracking-wider mb-2">
            過去のヒアリング
          </h4>
          {answeredHearings.map((h) => (
            <div key={h.id} className="mb-3 p-3 bg-gh-surface border border-gh-border rounded-lg text-sm">
              <div className="text-[10px] text-gh-text-muted mb-2">
                {h.phase === "pre_plan" ? "事前ヒアリング" : "計画中ヒアリング"} (Round {h.round})
              </div>
              {h.questions.map((q) => {
                const answer = h.answers?.find((a) => a.index === q.index);
                return (
                  <div key={q.index} className="mb-2">
                    <div className="text-gh-text-secondary font-medium">Q{q.index}: {q.question}</div>
                    <div className="text-gh-text ml-2">→ {answer?.answer || "未回答"}</div>
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      )}

      {/* 現在のヒアリング */}
      {pendingHearing && (
        <HearingForm
          taskId={taskId}
          hearing={pendingHearing}
          onAnswered={onAnswered}
        />
      )}
    </div>
  );
}

function HearingForm({
  taskId,
  hearing,
  onAnswered,
}: {
  taskId: string;
  hearing: TaskHearing;
  onAnswered: () => void;
}) {
  const [answers, setAnswers] = useState<Record<number, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const updateAnswer = (index: number, value: string) => {
    setAnswers((prev) => ({ ...prev, [index]: value }));
  };

  const allAnswered = hearing.questions.every((q) => answers[q.index]?.trim());

  const handleSubmit = async () => {
    if (!allAnswered) return;
    setSubmitting(true);
    setError(null);
    try {
      const answerList: HearingAnswer[] = hearing.questions.map((q) => ({
        index: q.index,
        answer: answers[q.index].trim(),
      }));
      await answerHearing(taskId, answerList);
      onAnswered();
    } catch (e) {
      setError(e instanceof Error ? e.message : "回答送信に失敗しました");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="p-4 bg-gh-surface border border-gh-orange/30 rounded-lg">
      <h4 className="text-sm font-semibold text-gh-orange mb-3">
        {hearing.phase === "pre_plan" ? "事前ヒアリング" : "計画に関する確認事項"}
      </h4>

      <div className="space-y-4">
        {hearing.questions.map((q) => (
          <div key={q.index}>
            <label className="block text-sm font-medium text-gh-text mb-1.5">
              Q{q.index}: {q.question}
            </label>
            {q.options && q.options.length > 0 ? (
              <div className="flex flex-wrap gap-2">
                {q.options.map((opt) => (
                  <button
                    key={opt}
                    type="button"
                    onClick={() => updateAnswer(q.index, opt)}
                    className={`px-3 py-1.5 text-sm rounded-md border transition ${
                      answers[q.index] === opt
                        ? "border-gh-blue bg-gh-blue/15 text-gh-blue"
                        : "border-gh-border text-gh-text-secondary hover:border-gh-text-muted hover:text-gh-text"
                    }`}
                  >
                    {opt}
                  </button>
                ))}
              </div>
            ) : (
              <textarea
                value={answers[q.index] || ""}
                onChange={(e) => updateAnswer(q.index, e.target.value)}
                rows={2}
                className="w-full px-3 py-2 bg-gh-canvas border border-gh-border rounded-md text-sm text-gh-text placeholder:text-gh-text-muted focus:outline-none focus:border-gh-blue focus:ring-1 focus:ring-gh-blue/40 resize-none"
                placeholder="回答を入力..."
              />
            )}
          </div>
        ))}
      </div>

      {error && <div className="text-gh-red text-sm mt-3">{error}</div>}

      <button
        onClick={handleSubmit}
        disabled={!allAnswered || submitting}
        className="mt-4 px-4 py-2 bg-gh-orange/90 text-white rounded-md hover:bg-gh-orange transition text-sm font-medium disabled:opacity-50"
      >
        {submitting ? "送信中..." : "回答を送信"}
      </button>
    </div>
  );
}
