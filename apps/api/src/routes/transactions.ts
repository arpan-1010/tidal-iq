import { Hono } from "hono";
import { transactionRepo } from "../db/repositories/txn.repo";

const router = new Hono();

router.get('/', async(c) => {
    try {
        const chain = c.req.query('chain');
        const limit = parseInt(c.req.query('limit') ?? '50');

        const txs = await transactionRepo.getRecent(limit, chain);
        return c.json({data: txs, count: txs.length});
    } catch (err) {
        console.error('Error fetching transactions: ', err);
        return c.json({error: 'Failed to fetch transactions'}, 500);
    }
});

router.get('/:hash', async (c) => {
    try {
        const hash = c.req.param('hash');
        const tx = await transactionRepo.getByHash(hash);

        if(!tx) {
            return c.json({error: 'Transaction not found'}, 404);
        }
        return c.json({data: tx});
    } catch (err) {
        console.error('Error fetching transaction: ', err);
        return c.json({error: 'Failed to fetch transaction'}, 500);
    }
});

export default router;