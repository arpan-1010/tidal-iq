"use client";

import { useEffect, useRef, useState, type FormEvent } from "react";
import { Send, ThumbsDown, ThumbsUp } from "lucide-react";
import {
  AI_DISCLAIMER,
  streamChat,
  type ChatSource,
  type Citation,
} from "@tidaliq/shared";
import { ai, API_URL } from "@/lib/ai";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { CHAIN_META, formatUsd } from "@/lib/format";
import { CitedText } from "@/components/cited-text";

interface Msg {
  id: number;
  role: "user" | "assistant";
  text: string;
  sources: ChatSource[];
  citations: Citation[];
  status: "streaming" | "done" | "error";
  messageId?: number;
  vote?: "up" | "down";
}

const SUGGESTIONS = [
  "Summarise today's biggest moves in two sentences",
  "Which wallet has been most active lately?",
  "Is there anything unusual on Ethereum right now?",
];

function friendlyError(message: string): string {
  if (message.includes("503")) return "AI chat is currently unavailable.";
  if (message.includes("429"))
    return "You're sending messages too fast. Please wait a moment.";
  return message;
}

export function ChatPanel() {
  const [msgs, setMsgs] = useState<Msg[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);

  const sessionRef = useRef<string | undefined>(undefined);
  const idRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = listRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [msgs]);

  useEffect(() => () => abortRef.current?.abort(), []);

  const send = async (raw: string) => {
    const text = raw.trim();
    if (!text || busy) return;

    const assistantId = ++idRef.current + 1000000;
    setMsgs((prev) => [
      ...prev,
      {
        id: ++idRef.current,
        role: "user",
        text,
        sources: [],
        citations: [],
        status: "done",
      },
      {
        id: assistantId,
        role: "assistant",
        text: "",
        sources: [],
        citations: [],
        status: "streaming",
      },
    ]);
    setInput("");
    setBusy(true);

    const patch = (fn: (m: Msg) => Msg) =>
      setMsgs((prev) => prev.map((m) => (m.id === assistantId ? fn(m) : m)));

    const ctrl = new AbortController();
    abortRef.current = ctrl;

    try {
      await streamChat(
        API_URL,
        text,
        sessionRef.current,
        (e) => {
          switch (e.event) {
            case "meta":
              sessionRef.current = e.data.session_id;
              patch((m) => ({ ...m, sources: e.data.sources }));
              break;
            case "token":
              patch((m) => ({ ...m, text: m.text + e.data.t }));
              break;
            case "done":
              patch((m) => ({
                ...m,
                text: e.data.text,
                citations: e.data.citations,
                messageId: e.data.message_id,
                status: "done",
              }));
              break;
            case "error":
              patch((m) => ({
                ...m,
                text: friendlyError(e.data.message),
                status: "error",
              }));
              break;
          }
        },
        ctrl.signal,
      );
    } catch (err) {
      if (!(err instanceof DOMException && err.name === "AbortError")) {
        patch((m) => ({
          ...m,
          text: "Couldn't reach the server. Please try again.",
          status: "error",
        }));
      }
    } finally {
      patch((m) =>
        m.status === "streaming"
          ? {
              ...m,
              status: m.text ? "done" : "error",
              text: m.text || "No response received.",
            }
          : m,
      );
      setBusy(false);
    }
  };

  const vote = async (m: Msg, helpful: boolean) => {
    if (m.messageId == null || m.vote) return;
    setMsgs((prev) =>
      prev.map((x) =>
        x.id === m.id ? { ...x, vote: helpful ? "up" : "down" } : x,
      ),
    );
    try {
      await ai.sendFeedback({
        kind: "chat",
        ref: String(m.messageId),
        helpful,
      });
    } catch {
    }
  };

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    void send(input);
  };

  return (
    <div className="space-y-3">
      <div ref={listRef} className="max-h-96 space-y-3 overflow-y-auto pr-1">
        {msgs.length === 0 && (
          <div className="space-y-2">
            <p className="text-sm text-slate-700">
              Ask about large transfers TidalIQ has tracked. Answers cite the
              transactions they use.
            </p>
            <div className="flex flex-wrap gap-2">
              {SUGGESTIONS.map((s) => (
                <button
                  key={s}
                  type="button"
                  onClick={() => void send(s)}
                  className="rounded-full border border-slate-500/50 bg-white/50 px-3 py-1 text-xs text-slate-800 transition-colors hover:bg-white/80"
                >
                  {s}
                </button>
              ))}
            </div>
          </div>
        )}

        {msgs.map((m) =>
          m.role === "user" ? (
            <div
              key={m.id}
              className="ml-auto w-fit max-w-[85%] rounded-xl bg-slate-900 px-3 py-2 text-sm text-white"
            >
              {m.text}
            </div>
          ) : (
            <div
              key={m.id}
              className="w-fit max-w-[92%] space-y-2 rounded-xl border border-slate-500/30 bg-white/50 px-3 py-2"
            >
              <p
                className={`whitespace-pre-wrap text-sm ${m.status === "error" ? "text-red-700" : "text-slate-900"}`}
              >
                {m.text ? (
                  <CitedText text={m.text} sources={m.sources} />
                ) : (
                  <span className="text-slate-600">Thinking…</span>
                )}
                {m.status === "streaming" && m.text && (
                  <span className="ml-0.5 animate-pulse">▍</span>
                )}
              </p>

              {m.status === "done" && m.citations.length > 0 && (
                <ul className="space-y-0.5 border-t border-slate-500/30 pt-2 text-[11px] text-slate-700">
                  {m.citations.map((c) => (
                    <li key={c.n}>
                      <a
                        href={c.explorer_url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="hover:text-primary hover:underline"
                      >
                        [{c.n}] {CHAIN_META[c.chain]?.label ?? c.chain}{" "}
                        {c.token}
                        {c.value_usd != null &&
                          ` · ${formatUsd(c.value_usd)}`}{" "}
                        · {c.time}
                      </a>
                    </li>
                  ))}
                </ul>
              )}

              {m.status === "done" && m.messageId != null && (
                <div className="flex items-center gap-1 text-slate-600">
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    disabled={!!m.vote}
                    onClick={() => void vote(m, true)}
                    aria-label="Helpful"
                    className={m.vote === "up" ? "text-emerald-700" : ""}
                  >
                    <ThumbsUp className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    disabled={!!m.vote}
                    onClick={() => void vote(m, false)}
                    aria-label="Not helpful"
                    className={m.vote === "down" ? "text-red-700" : ""}
                  >
                    <ThumbsDown className="h-3.5 w-3.5" />
                  </Button>
                </div>
              )}
            </div>
          ),
        )}
      </div>

      <form onSubmit={onSubmit} className="flex max-w-2xl gap-2">
        <Input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          maxLength={500}
          disabled={busy}
          placeholder="Ask about whale transactions…"
          className="h-8 border-slate-500/60 bg-white/60 text-sm placeholder:text-slate-600"
        />
        <Button
          type="submit"
          size="icon-sm"
          disabled={busy || !input.trim()}
          aria-label="Send"
        >
          <Send className="h-4 w-4" />
        </Button>
      </form>

      <p className="text-[11px] text-slate-600">{AI_DISCLAIMER}</p>
    </div>
  );
}