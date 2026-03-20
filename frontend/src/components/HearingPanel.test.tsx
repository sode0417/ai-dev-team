import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, cleanup } from "@testing-library/react";
import { HearingPanel } from "./HearingPanel";
import type { TaskHearing } from "@/types";

vi.mock("@/lib/api", () => ({
  answerHearing: vi.fn().mockResolvedValue({}),
}));

afterEach(() => {
  cleanup();
});

function makeHearing(overrides: Partial<TaskHearing> = {}): TaskHearing {
  return {
    id: "h1",
    task_id: "t1",
    session_id: null,
    phase: "pre_plan",
    round: 1,
    questions: [
      { index: 1, question: "言語は？", options: ["Rust", "TypeScript", "Python"] },
      { index: 2, question: "補足事項は？" },
    ],
    answers: null,
    status: "pending",
    created_at: "2026-03-20T00:00:00Z",
    ...overrides,
  };
}

describe("HearingPanel", () => {
  it("options ありの質問でボタンとテキスト入力の両方が表示される", () => {
    render(<HearingPanel taskId="t1" hearings={[makeHearing()]} onAnswered={() => {}} />);

    expect(screen.getAllByRole("button", { name: "Rust" }).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByRole("button", { name: "TypeScript" }).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByRole("button", { name: "Python" }).length).toBeGreaterThanOrEqual(1);

    const textareas = screen.getAllByRole("textbox");
    expect(textareas).toHaveLength(2);
  });

  it("ボタンクリックで textarea に値がセットされる", () => {
    render(<HearingPanel taskId="t1" hearings={[makeHearing()]} onAnswered={() => {}} />);

    fireEvent.click(screen.getAllByRole("button", { name: "Rust" })[0]);

    const textareas = screen.getAllByRole("textbox");
    expect(textareas[0]).toHaveValue("Rust");
  });

  it("ボタン選択後に textarea を編集できる", () => {
    render(<HearingPanel taskId="t1" hearings={[makeHearing()]} onAnswered={() => {}} />);

    fireEvent.click(screen.getAllByRole("button", { name: "Rust" })[0]);
    const textarea = screen.getAllByRole("textbox")[0];
    fireEvent.change(textarea, { target: { value: "Rust + Go" } });

    expect(textarea).toHaveValue("Rust + Go");
  });

  it("ボタンを使わず直接テキスト入力できる", () => {
    render(<HearingPanel taskId="t1" hearings={[makeHearing()]} onAnswered={() => {}} />);

    const textarea = screen.getAllByRole("textbox")[0];
    fireEvent.change(textarea, { target: { value: "カスタム回答" } });

    expect(textarea).toHaveValue("カスタム回答");
  });

  it("1000文字の入力は許可され、1001文字は拒否される", () => {
    render(<HearingPanel taskId="t1" hearings={[makeHearing()]} onAnswered={() => {}} />);

    const textarea = screen.getAllByRole("textbox")[0];

    // 1000文字はOK
    const text1000 = "a".repeat(1000);
    fireEvent.change(textarea, { target: { value: text1000 } });
    expect(textarea).toHaveValue(text1000);

    // 1001文字は拒否（1000文字のまま）
    const text1001 = "a".repeat(1001);
    fireEvent.change(textarea, { target: { value: text1001 } });
    expect(textarea).toHaveValue(text1000);
  });

  it("textarea に maxLength 属性が設定されている", () => {
    render(<HearingPanel taskId="t1" hearings={[makeHearing()]} onAnswered={() => {}} />);

    const textareas = screen.getAllByRole("textbox");
    textareas.forEach((ta) => {
      expect(ta).toHaveAttribute("maxlength", "1000");
    });
  });

  it("全質問回答済みで送信ボタンが有効になる", () => {
    render(<HearingPanel taskId="t1" hearings={[makeHearing()]} onAnswered={() => {}} />);

    const submitButton = screen.getByRole("button", { name: "回答を送信" });
    expect(submitButton).toBeDisabled();

    const textareas = screen.getAllByRole("textbox");
    fireEvent.change(textareas[0], { target: { value: "Rust" } });
    fireEvent.change(textareas[1], { target: { value: "特になし" } });

    expect(submitButton).toBeEnabled();
  });

  it("options なしの質問が従来通り textarea のみ表示される", () => {
    const hearing = makeHearing({
      questions: [{ index: 1, question: "自由回答" }],
    });
    render(<HearingPanel taskId="t1" hearings={[hearing]} onAnswered={() => {}} />);

    expect(screen.getAllByRole("textbox")).toHaveLength(1);
    expect(screen.getByPlaceholderText("回答を入力...")).toBeInTheDocument();
  });

  it("options ありの質問の placeholder が適切に表示される", () => {
    render(<HearingPanel taskId="t1" hearings={[makeHearing()]} onAnswered={() => {}} />);

    expect(screen.getByPlaceholderText("選択肢をクリックするか、自由に入力...")).toBeInTheDocument();
  });

  it("送信ボタンクリックで answerHearing が呼ばれる", async () => {
    const { answerHearing } = await import("@/lib/api");
    const onAnswered = vi.fn();

    render(<HearingPanel taskId="t1" hearings={[makeHearing()]} onAnswered={onAnswered} />);

    const textareas = screen.getAllByRole("textbox");
    fireEvent.change(textareas[0], { target: { value: "Rust" } });
    fireEvent.change(textareas[1], { target: { value: "特になし" } });

    fireEvent.click(screen.getByRole("button", { name: "回答を送信" }));

    await waitFor(() => {
      expect(answerHearing).toHaveBeenCalledWith("t1", [
        { index: 1, answer: "Rust" },
        { index: 2, answer: "特になし" },
      ]);
      expect(onAnswered).toHaveBeenCalled();
    });
  });
});
