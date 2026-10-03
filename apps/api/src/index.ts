import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger } from "hono/logger";
import transactionRouter from "./routes/transactions";
import { blockChainServices } from "./services/blockchain.service";
    
const app = new Hono();

app.use('*', cors({
    origin: process.env.FRONTEND_URL ?? 'http://localhost:3000',
    credentials: true,
}));

app.use('*', logger());

app.get('/health', (c) => c.json({
    status: 'ok',
    timestamp: Date.now()
}));

app.get('/', (c) => c.json({
    message: 'Whale Alert API'
}));

app.route('/transactions', transactionRouter);

const port = parseInt(process.env.PORT ?? '4000');

blockChainServices.start().catch((err) => {
    console.error('Blockchain service failed to start: ', err);  
});

console.log(`API running on http://localhost:${port}`);

export default {port, fetch: app.fetch};