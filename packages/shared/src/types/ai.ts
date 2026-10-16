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