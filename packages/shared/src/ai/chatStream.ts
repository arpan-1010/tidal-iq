import type { ChatStreamEvent } from "../types/ai";

export async function streamChat(
  apiBase: string,
  message: string,
  sessionId: string | undefined,
  onEvent: (e: ChatStreamEvent) => void,
  signal?: AbortSignal,
): Promise<void> {
  const res = await fetch(`${apiBase}/ai/chat/stream`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ message, session_id: sessionId }),
    signal,
  });
  if (!res.ok || !res.body) {
    onEvent({ event: "error", data: { message: `Request failed (${res.status})` } });
    return;
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = "";

  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });

    // SSE frames are separated by a blank line
    let idx: number;
    while ((idx = buf.indexOf("\n\n")) !== -1) {
      const frame = buf.slice(0, idx);
      buf = buf.slice(idx + 2);

      let event = "message";
      let data = "";
      for (const line of frame.split("\n")) {
        if (line.startsWith("event:")) event = line.slice(6).trim();
        else if (line.startsWith("data:")) data += line.slice(5).trim();
      }
      if (!data) continue; // keep-alive comments
      try {
        onEvent({ event, data: JSON.parse(data) } as ChatStreamEvent);
      } catch {
        /* ignore a malformed frame */
      }
    }
  }
}