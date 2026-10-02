import { eq, desc, gte, and, gt } from "drizzle-orm";
import { db } from '../client';
import { transactions } from "../schema";
import type { WhaleTransaction } from "@whale/shared";
import { randomUUID } from "crypto";

export const transactionRepo = {
    async create(tx: Omit<WhaleTransaction, 'id'>) :Promise<string> {
        const id = randomUUID();
        await db.insert(transactions).values({
            id,
            hash: tx.hash,
            chain: tx.chain,
            fromAddress: tx.from,
            toAddress: tx.to,
            value: tx.value,
            valueUsd: tx.valueUsd.toString(),
            token: tx.token,
            tokenSymbol: tx.tokenSymbol,
            blockNumber: Number(tx.blockNumber),
            txTimestamp: new Date(Number(tx.timestamp) * 1000),
        });

        return id;
    },

    async getRecent(limit = 50, chain ?: string) {
        const conditions = chain ? [eq(transactions.chain, chain)] : [];

        return db
            .select()
            .from(transactions)
            .where(conditions.length ? and (...conditions) : undefined)
            .orderBy(desc(transactions.txTimestamp))
            .limit(limit);
    },

    async getByHash(hash: string) {
        const result = await db
            .select()
            .from(transactions)
            .where(eq(transactions.hash, hash))
            .limit(1);

        return result[0] ?? null;
    },

    async updateAIFields(
        id: string,
        fields: {
            aiSummary ?: string;
            impactScore ?: number;
            isAnomaly ?: boolean;
            anomalyReason ?: string;
        }
    ) {
        await db
            .update(transactions)
            .set(fields)
            .where(eq(transactions.id, id));
    },

    async getLargeTransactions(minUsd : number, chain ?: string) {
        const conditions = [
            gte(transactions.valueUsd, minUsd.toString()),
            ...(chain ? [eq(transactions.chain, chain)] : []),
        ];
        return db
            .select()
            .from(transactions)
            .where(and(...conditions))
            .orderBy(desc(transactions.valueUsd))
            .limit(100);
    },
};