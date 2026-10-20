import { createAiClient } from "@tidaliq/shared";

export const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:4000";
export const ai = createAiClient(API_URL);