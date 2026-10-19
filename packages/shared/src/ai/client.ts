import type {
  AISummary,
  FeedbackRequest,
  Narrative,
  SearchResponse,
  WalletProfile,
} from "../types/ai";

async function getJson<T>(url: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(url, { signal });
  if (!res.ok) throw new Error(`Request failed (${res.status})`);
  return res.json() as Promise<T>;
}

export function createAiClient(apiBase: string) {
  return {
    /** Returns null when the transaction has no analysis yet (404). */
    async getAnalysis(hash: string, signal?: AbortSignal): Promise<AISummary | null> {
      const res = await fetch(`${apiBase}/ai/transactions/${encodeURIComponent(hash)}`, { signal });
      if (res.status === 404) return null;
      if (!res.ok) throw new Error(`Request failed (${res.status})`);
      return ((await res.json()) as { data: AISummary }).data;
    },

    search(query: string, limit = 10, signal?: AbortSignal): Promise<SearchResponse> {
      const q = encodeURIComponent(query);
      return getJson<SearchResponse>(`${apiBase}/ai/search?q=${q}&limit=${limit}`, signal);
    },

    /** Returns null when the wallet isn't in the tracked data (404). */
    async getWalletProfile(chain: string, address: string, signal?: AbortSignal): Promise<WalletProfile | null> {
      const url = `${apiBase}/ai/wallets/${encodeURIComponent(chain)}/${encodeURIComponent(address)}`;
      const res = await fetch(url, { signal });
      if (res.status === 404) return null;
      if (!res.ok) throw new Error(`Request failed (${res.status})`);
      return ((await res.json()) as { data: WalletProfile }).data;
    },

    /** Returns null when there are no transactions in the window. */
    async getNarrative(hours = 6, signal?: AbortSignal): Promise<Narrative | null> {
      const body = await getJson<{ data: Narrative | null }>(`${apiBase}/ai/narrative?hours=${hours}`, signal);
      return body.data;
    },

    async sendFeedback(fb: FeedbackRequest): Promise<void> {
      const res = await fetch(`${apiBase}/ai/feedback`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(fb),
      });
      if (!res.ok) throw new Error(`Request failed (${res.status})`);
    },
  };
}