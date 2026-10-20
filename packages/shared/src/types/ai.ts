export interface AISummary {
  transactionId: string;
  summary: string;
  impactScore: number;
  isAnomaly: boolean;
  anomalyReason?: string;
  generatedAt: number;
}

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  timestamp: number;
}

export const AI_DISCLAIMER = "AI-generated insights, not financial advice.";

export interface Citation {
  n: number;
  hash: string;
  chain: string;
  token: string;
  value_usd: number | null;
  time: string;
  summary: string | null;
  explorer_url: string;
}

export type AnswerSource = "llm" | "template" | "cache";

export interface ChatAnswer {
  session_id: string;
  message_id: number;
  text: string;
  citations: Citation[];
  source: AnswerSource;
  model: string | null;
  disclaimer: string;
}

export interface ChatSource {
  n: number;
  chain: string;
  token: string;
  value_usd: number | null;
  time: string;
  explorer_url: string;
}

export interface WalletProfile {
  chain: string;
  address: string;
  label: string | null;
  profile: string;
  stats: {
    sent_count: number;
    sent_total_usd: number;
    sent_avg_usd: number;
    sent_largest_usd: number;
    sent_last_30d: number;
    first_seen: string | null;
    last_seen: string | null;
    received_count: number;
    received_total_usd: number;
    top_tokens: { token: string; count: number; total_usd: number }[];
  };
  source: AnswerSource;
  model: string | null;
  updated_at: string | null;
  disclaimer: string;
}

export interface Narrative {
  id: number | null;
  window_hours: number;
  text: string;
  citations: Citation[];
  stats: {
    chain: string;
    count: number;
    total_usd: number;
    largest_usd: number;
  }[];
  source: AnswerSource;
  model: string | null;
  created_at: string | null;
  disclaimer: string;
}

export type FeedbackKind =
  | "summary"
  | "chat"
  | "search"
  | "narrative"
  | "profile";

export interface FeedbackRequest {
  kind: FeedbackKind;
  ref: string;
  helpful: boolean;
  comment?: string;
}

export type ChatStreamEvent =
  | { event: "meta"; data: { session_id: string; sources: ChatSource[] } }
  | { event: "token"; data: { t: string } }
  | { event: "done"; data: ChatAnswer }
  | { event: "error"; data: { message: string } };

export type ImportanceTier = "Low" | "Medium" | "High" | "Critical";

export type AnomalyFlagCode =
  | "UNUSUAL_SIZE"
  | "FIRST_LARGE_MOVE"
  | "RAPID_SUCCESSION"
  | "TOKEN_OUTLIER";

export interface AISummary {
  tx_hash: string;
  summary: string;
  importance_score: number;
  importance_tier: ImportanceTier;
  importance_reason: string | null;
  anomaly_flags: { code: AnomalyFlagCode; detail: string }[];
  anomaly_note: string | null;
  source: "llm" | "template";
  model: string | null;
  prompt_version: string | null;
  explorer_url: string;
  disclaimer: string;
}

export interface SearchFilters {
  anomaly_only: boolean;
  chains: string[] | null;
  flags: AnomalyFlagCode[] | null;
  max_usd: number | null;
  min_usd: number | null;
  period: string | null;
  semantic_text: string | null;
  symbols: string[] | null;
}

export interface SearchResult {
  chain: string;
  token: string;
  hash: string;
  from: string;
  to: string;
  value_usd: number | null;
  time: string;
  summary: string | null;
  summary_pending: boolean;
  importance_score: number | null;
  importance_tier: ImportanceTier | null;
  similarity: number | null;
  explorer_url: string;
}

export interface SearchResponse {
  query: string;
  count: number;
  ai_used: boolean;
  semantic: boolean;
  filters: SearchFilters;
  results: SearchResult[];
  available_tokens: string[] | null;
  note: string | null;
  disclaimer: string;
}