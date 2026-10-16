export interface WhaleTransaction {
  id: number | string;
  hash: string;
  chain: string;
  from_address: string;
  to_address: string;
  value: string;
  value_usd: number;
  token: string;
  token_symbol: string;
  block_number: number;
  tx_timestamp: string;
  ai_summary: string | null;
  impact_score: number | null;
  is_anomaly: boolean | null;
  anomaly_reason: string | null;
}