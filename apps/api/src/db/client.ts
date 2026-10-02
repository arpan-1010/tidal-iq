import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";
import * as schema from "./schema";

const connStr = process.env.DATABASE_URL;

if(!connStr) {
    throw new Error("DATABASE_URL is not found");
}

const client = postgres(connStr, {max: 10});

export const db = drizzle(client, {schema});

export type DB = typeof db;