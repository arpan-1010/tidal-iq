import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { logger } from 'hono/logger';

const app = new Hono();

app.use('*', cors({
    origin: process.env.FRONTEND_URL ?? 'http://localhost:3000',
    credentials: true,
}));

app.use('*', logger());

app.get('/health', (c) => c.json({status: 'ok', timestamp: Date.now()}));

const port = parseInt(process.env.PORT ?? '4000');

console.log(`API is running on http://localhost:${port}`);

export default {
    port,
    fetch: app.fetch,
}