"use client";

import { useEffect, useState } from "react";
import { fetchRepositoryIssues, createRepositoryIssue } from "@/lib/api";
import type { GitHubIssue } from "@/types";

export function IssueList({
  projectId,
  repoId,
  onExecute,
}: {
  projectId: string;
  repoId: string;
  onExecute?: (issue: GitHubIssue) => void;
}) {
  const [issues, setIssues] = useState<GitHubIssue[]>([]);
  const [state, setState] = useState<"open" | "closed">("open");
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(false);
  const [showForm, setShowForm] = useState(false);

  const load = (p: number, s: string, append = false) => {
    setLoading(true);
    fetchRepositoryIssues(projectId, repoId, { state: s, page: p, per_page: 20 })
      .then((res) => {
        setIssues((prev) => (append ? [...prev, ...res.data] : res.data));
        setHasMore(res.data.length === 20);
      })
      .catch(() => setIssues([]))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    setPage(1);
    load(1, state);
  }, [state, projectId, repoId]);

  const loadMore = () => {
    const next = page + 1;
    setPage(next);
    load(next, state, true);
  };

  return (
    <div className="mt-3">
      <div className="flex items-center gap-2 mb-2">
        <div className="flex gap-1">
          <button
            onClick={() => setState("open")}
            className={`text-xs px-2.5 py-1 rounded-md font-medium transition ${
              state === "open"
                ? "bg-gh-green/15 text-gh-green"
                : "text-gh-text-secondary hover:text-gh-text"
            }`}
          >
            Open
          </button>
          <button
            onClick={() => setState("closed")}
            className={`text-xs px-2.5 py-1 rounded-md font-medium transition ${
              state === "closed"
                ? "bg-gh-purple/15 text-gh-purple"
                : "text-gh-text-secondary hover:text-gh-text"
            }`}
          >
            Closed
          </button>
        </div>
        <button
          onClick={() => setShowForm(!showForm)}
          className="ml-auto text-xs text-gh-link hover:underline"
        >
          {showForm ? "閉じる" : "+ New Issue"}
        </button>
      </div>

      {showForm && (
        <CreateIssueForm
          projectId={projectId}
          repoId={repoId}
          onCreated={() => {
            setShowForm(false);
            setState("open");
            setPage(1);
            load(1, "open");
          }}
        />
      )}

      {loading && issues.length === 0 ? (
        <p className="text-gh-text-muted text-xs py-2">読み込み中...</p>
      ) : issues.length === 0 ? (
        <p className="text-gh-text-muted text-xs py-2">Issues はありません</p>
      ) : (
        <div>
          {issues.map((issue, i) => (
            <div
              key={issue.number}
              className={`flex items-start gap-2.5 px-2 py-2.5 hover:bg-gh-overlay rounded-md transition ${
                i > 0 ? "border-t border-gh-border-muted" : ""
              }`}
            >
              {/* Issue icon */}
              <svg className={`w-4 h-4 mt-0.5 shrink-0 ${state === "open" ? "text-gh-green" : "text-gh-purple"}`} viewBox="0 0 16 16" fill="currentColor">
                {state === "open" ? (
                  <path d="M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Z" />
                ) : (
                  <path d="M11.28 6.78a.75.75 0 0 0-1.06-1.06L7.25 8.69 5.78 7.22a.75.75 0 0 0-1.06 1.06l2 2a.75.75 0 0 0 1.06 0l3.5-3.5ZM16 8A8 8 0 1 1 0 8a8 8 0 0 1 16 0Zm-1.5 0a6.5 6.5 0 1 0-13 0 6.5 6.5 0 0 0 13 0Z" />
                )}
              </svg>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1.5 flex-wrap">
                  <a
                    href={issue.html_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-sm font-medium text-gh-text hover:text-gh-link"
                  >
                    {issue.title}
                  </a>
                  {issue.labels.map((label) => (
                    <span
                      key={label.name}
                      className="text-[10px] px-1.5 py-0.5 rounded-full font-medium border"
                      style={{
                        backgroundColor: `#${label.color}18`,
                        color: `#${label.color}`,
                        borderColor: `#${label.color}30`,
                      }}
                    >
                      {label.name}
                    </span>
                  ))}
                </div>
                <div className="text-[11px] text-gh-text-muted mt-0.5">
                  #{issue.number} · {issue.user?.login} · {new Date(issue.created_at).toLocaleDateString("ja-JP")}
                  {issue.comments > 0 && (
                    <span className="ml-2 inline-flex items-center gap-0.5">
                      <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M12 20.25c4.97 0 9-3.694 9-8.25s-4.03-8.25-9-8.25S3 7.444 3 12c0 2.104.859 4.023 2.273 5.48.432.447.74 1.04.586 1.641a4.483 4.483 0 0 1-.923 1.785A5.969 5.969 0 0 0 6 21c1.282 0 2.47-.402 3.445-1.087.81.22 1.668.337 2.555.337Z" />
                      </svg>
                      {issue.comments}
                    </span>
                  )}
                </div>
              </div>
              {onExecute && state === "open" && (
                <button
                  onClick={(e) => { e.stopPropagation(); onExecute(issue); }}
                  className="shrink-0 px-2 py-1 text-xs font-medium rounded-md bg-gh-blue/15 text-gh-blue hover:bg-gh-blue/25 transition"
                  title={`Issue #${issue.number} \u3092\u5b9f\u884c`}
                >
                  Execute
                </button>
              )}
            </div>
          ))}
          {hasMore && (
            <button
              onClick={loadMore}
              disabled={loading}
              className="text-xs text-gh-link hover:underline disabled:opacity-50 mt-2 ml-2"
            >
              {loading ? "読み込み中..." : "もっと表示"}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function CreateIssueForm({
  projectId,
  repoId,
  onCreated,
}: {
  projectId: string;
  repoId: string;
  onCreated: () => void;
}) {
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [labels, setLabels] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      const labelList = labels
        .split(",")
        .map((l) => l.trim())
        .filter(Boolean);
      await createRepositoryIssue(projectId, repoId, {
        title: title.trim(),
        body: body.trim() || undefined,
        labels: labelList.length > 0 ? labelList : undefined,
      });
      onCreated();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Issue 作成に失敗しました");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="mb-4 p-3 bg-gh-surface border border-gh-border rounded-lg space-y-3">
      <div>
        <input
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Issue タイトル"
          className="w-full px-3 py-2 bg-gh-canvas border border-gh-border rounded-md text-sm text-gh-text placeholder:text-gh-text-muted focus:outline-none focus:border-gh-blue focus:ring-1 focus:ring-gh-blue/40"
          required
        />
      </div>
      <div>
        <textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          placeholder="本文（任意）"
          rows={4}
          className="w-full px-3 py-2 bg-gh-canvas border border-gh-border rounded-md text-sm text-gh-text placeholder:text-gh-text-muted focus:outline-none focus:border-gh-blue focus:ring-1 focus:ring-gh-blue/40 resize-none"
        />
      </div>
      <div>
        <input
          type="text"
          value={labels}
          onChange={(e) => setLabels(e.target.value)}
          placeholder="ラベル（カンマ区切り、例: bug, enhancement）"
          className="w-full px-3 py-2 bg-gh-canvas border border-gh-border rounded-md text-sm text-gh-text placeholder:text-gh-text-muted focus:outline-none focus:border-gh-blue focus:ring-1 focus:ring-gh-blue/40"
        />
      </div>
      {error && <div className="text-gh-red text-sm">{error}</div>}
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={!title.trim() || submitting}
          className="px-4 py-1.5 text-sm font-medium rounded-md bg-gh-green text-white hover:bg-gh-green/90 transition disabled:opacity-50"
        >
          {submitting ? "作成中..." : "Issue を作成"}
        </button>
      </div>
    </form>
  );
}
