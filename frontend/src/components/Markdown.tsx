"use client";

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

export function Markdown({ children }: { children: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        h1: ({ children }) => (
          <h1 className="text-lg font-bold mt-4 mb-2 text-gh-text border-b border-gh-border pb-1">
            {children}
          </h1>
        ),
        h2: ({ children }) => (
          <h2 className="text-base font-semibold mt-3 mb-1.5 text-gh-text">
            {children}
          </h2>
        ),
        h3: ({ children }) => (
          <h3 className="text-sm font-semibold mt-2 mb-1 text-gh-text">
            {children}
          </h3>
        ),
        h4: ({ children }) => (
          <h4 className="text-sm font-medium mt-2 mb-1 text-gh-text-secondary">
            {children}
          </h4>
        ),
        p: ({ children }) => (
          <p className="text-sm leading-relaxed mb-2 text-gh-text">{children}</p>
        ),
        ul: ({ children }) => (
          <ul className="list-disc list-inside text-sm mb-2 space-y-0.5 text-gh-text">
            {children}
          </ul>
        ),
        ol: ({ children }) => (
          <ol className="list-decimal list-inside text-sm mb-2 space-y-0.5 text-gh-text">
            {children}
          </ol>
        ),
        li: ({ children }) => <li className="text-sm text-gh-text">{children}</li>,
        code: ({ className, children }) => {
          const isBlock = className?.includes("language-");
          if (isBlock) {
            return (
              <code className="block p-3 bg-gh-canvas border border-gh-border rounded-md text-xs font-mono overflow-x-auto whitespace-pre mb-2">
                {children}
              </code>
            );
          }
          return (
            <code className="px-1 py-0.5 bg-gh-canvas border border-gh-border rounded text-xs font-mono text-gh-text">
              {children}
            </code>
          );
        },
        pre: ({ children }) => <pre className="mb-2">{children}</pre>,
        table: ({ children }) => (
          <div className="overflow-x-auto mb-2">
            <table className="text-sm border-collapse w-full">{children}</table>
          </div>
        ),
        thead: ({ children }) => (
          <thead className="bg-gh-surface">{children}</thead>
        ),
        th: ({ children }) => (
          <th className="border border-gh-border px-2 py-1 text-left text-xs font-semibold text-gh-text-secondary">
            {children}
          </th>
        ),
        td: ({ children }) => (
          <td className="border border-gh-border px-2 py-1 text-xs text-gh-text">
            {children}
          </td>
        ),
        blockquote: ({ children }) => (
          <blockquote className="border-l-2 border-gh-border pl-3 my-2 text-sm text-gh-text-secondary italic">
            {children}
          </blockquote>
        ),
        hr: () => <hr className="border-gh-border my-3" />,
        strong: ({ children }) => (
          <strong className="font-semibold text-gh-text">{children}</strong>
        ),
        a: ({ href, children }) => (
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="text-gh-link hover:underline"
          >
            {children}
          </a>
        ),
      }}
    >
      {children}
    </ReactMarkdown>
  );
}
