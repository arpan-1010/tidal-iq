import { pgTable, text, numeric, integer, boolean, timestamp, real, index } from "drizzle-orm/pg-core";

export const transactions = pgTable(
    'transactions',
    {
        id: text('id').primaryKey(),
        hash: text('hash').notNull().unique(),
        chain: text('chain').notNull(),
        fromAddress: text('from_address').notNull(),
        toAddress: text('to_address').notNull(),
        value: text('value').notNull(),
        valueUsd: numeric('value_usd', {precision: 20, scale: 4}),
        token: text('token').notNull(),
        tokenSymbol: text('token_symbol').notNull(),
        blockNumber: integer('block_number').notNull(),
        txTimestamp: timestamp('tx_timestamp').notNull(),
        createdAt: timestamp('created_at').defaultNow().notNull(),

        aiSummary: text('ai_summary'),
        impactScore: real('impact_score'),
        isAnomaly: boolean('is_anomaly').default(false),
        anomalyReason: text('anomaly_reason'),
    },
    (table) => [
        index('idx_tx_chain').on(table.chain),
        index('idx_tx_timestamp').on(table.txTimestamp),
        index('idx_tx_from').on(table.fromAddress),
        index('idx_tx_to').on(table.toAddress),
    ]
);

export const wallets = pgTable(
    'wallets', 
    {
        address: text('address').primaryKey(),
        chain: text('chain').notNull(),
        label: text('label'),
        isExchange: boolean('is_exchange').default(false),
        isWhale: boolean('is_whale').default(false),
        totalVolumeUsd: numeric('total_volume_usd', {precision: 30, scale: 4}).default('0'),
        transactionCount: integer('transaction_count').default(0),
        firstSeen: timestamp('first_seen').defaultNow(),
        lastSeen: timestamp('last_seen').defaultNow(),
        aiProfile: text('ai_profile'),
        updatedAt: timestamp('updated_at').defaultNow().notNull(),
    }
);

export const alerts = pgTable(
    'alerts',
    {
        id: text('id').primaryKey(),
        userId: text('user_id').notNull(),
        chain: text('chain'),
        token: text('token'),
        minValueUsd: numeric('min_value_usd', {precision: 20, scale: 4}),
        walletAddress: text('wallet_address'),
        isActive: boolean('is_active').default(false),
        createdAt: timestamp('created_at').defaultNow().notNull(),
    }
);

export const alertEvents = pgTable(
    'alert_events',
    {
        id: text('id').primaryKey(),
        alertId : text('alert_id').notNull().references(() => alerts.id, {onDelete: 'cascade'}),
        triggeredAt: timestamp('triggered_at').defaultNow().notNull(),
        notified: boolean('notified').default(false),
    }
);