"use client";

import { useEffect, useState } from "react";
import { fetchRepositoryPulls } from "@/lib/api";
import type { GitHubPullRequest } from "@/types";

export function PullRequestList({
  projectId,
  repoId,
}: {
  projectId: string;
  repoId: string;
}) {
  const [pulls, setPulls] = useState<GitHubPullRequest[]>([]);
  const [state, setState] = useState<"open" | "closed">("open");
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(false);

  const load = (p: number, s: string, append = false) => {
    setLoading(true);
    fetchRepositoryPulls(projectId, repoId, { state: s, page: p, per_page: 20 })
      .then((res) => {
        setPulls((prev) => (append ? [...prev, ...res.data] : res.data));
        setHasMore(res.data.length === 20);
      })
      .catch(() => setPulls([]))
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
      <div className="flex gap-1 mb-2">
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

      {loading && pulls.length === 0 ? (
        <p className="text-gh-text-muted text-xs py-2">読み込み中...</p>
      ) : pulls.length === 0 ? (
        <p className="text-gh-text-muted text-xs py-2">Pull Requests はありません</p>
      ) : (
        <div>
          {pulls.map((pr, i) => (
            <a
              key={pr.number}
              href={pr.html_url}
              target="_blank"
              rel="noopener noreferrer"
              className={`flex items-start gap-2.5 px-2 py-2.5 hover:bg-gh-overlay rounded-md transition ${
                i > 0 ? "border-t border-gh-border-muted" : ""
              }`}
            >
              {/* PR icon */}
              <svg className={`w-4 h-4 mt-0.5 shrink-0 ${state === "open" ? "text-gh-green" : "text-gh-purple"}`} viewBox="0 0 16 16" fill="currentColor">
                <path d="M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354ZM3.75 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm8.25.75a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Z" />
              </svg>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1.5">
                  <span className="text-sm font-medium text-gh-text hover:text-gh-link">
                    {pr.title}
                  </span>
                  {pr.draft && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded-full font-medium bg-gh-text-muted/20 text-gh-text-muted border border-gh-text-muted/30">
                      Draft
                    </span>
                  )}
                </div>
                <div className="text-[11px] text-gh-text-muted mt-0.5">
                  #{pr.number} · {pr.user?.login} ·{" "}
                  <code className="text-[10px] px-1 py-0.5 rounded bg-gh-blue/10 text-gh-blue">
                    {pr.head.ref}
                  </code>
                  {" "}→{" "}
                  <code className="text-[10px] px-1 py-0.5 rounded bg-gh-border/50 text-gh-text-secondary">
                    {pr.base.ref}
                  </code>
                  {" "}· {new Date(pr.created_at).toLocaleDateString("ja-JP")}
                </div>
              </div>
            </a>
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
