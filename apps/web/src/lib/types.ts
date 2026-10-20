export interface Transaction {
  id: string;
  hash: string;
  chain: string;
  from_address: string;
  to_address: string;
  value: string;
  token_symbol: string;
  value_usd: number;
  tx_timestamp: string;
  impact_score: number | null;
  is_anomaly: boolean | null;
  anomaly_reason: string | null;
  ai_summary: string | null;
}

export interface ChainStat {
  chain: string;
  count: number;
  volume_usd: number | null;
}

export interface PricePoint {
  t: number;
  price: number;
}

export interface VolumePoint {
  bucket: string;
  volume_usd: number;
  tx_count: number;
}

export interface InsightRow {
  hash: string;
  chain: string;
  token_symbol: string;
  value_usd: number;
  impact_score: number | null;
  is_anomaly: boolean | null;
  anomaly_reason: string | null;
  ai_summary: string | null;
  tx_timestamp: string;
}

export interface Insights {
  top: InsightRow[];
  anomalies: InsightRow[];
}