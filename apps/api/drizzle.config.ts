import type { Config } from "drizzle-kit";
import { config } from "dotenv";
import path, { dirname } from 'path';

config({path: path.resolve(__dirname, '../../.env')});

export default {
    schema: './src/db/schema.ts',
    out: './drizzle',
    dialect: 'postgresql',
    dbCredentials: {
        url: process.env.DATABASE_URL!,
    },
} satisfies Config;