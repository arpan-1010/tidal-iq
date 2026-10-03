import axios from 'axios';

const cache = new Map<string, {price: number, expiresAt: number}>();
const CACHE_TTL_MS = 60 * 1000;

const COINGEKO_IDS: Record<string, string> = {
    ETH: 'ethereum',
    BTC: 'bitcoin',
    USDT: 'tether',
    USDC: 'usd-coin',
    DAI: 'dai',
};

export const priceService = {
    async getUsdPrice(symbol: string): Promise<number> {
        const upper = symbol.toUpperCase();
        
        if(['USDC', 'USDT', 'DAI', 'BUSD'].includes(upper)) return 1;

        const cached = cache.get(upper);
        if(cached && cached.expiresAt > Date.now()) return cached.price;

        const coinId = COINGEKO_IDS[upper];
        if(!coinId) {
            console.warn(`No CoinGeko ID for ${upper}, defaulting to 0`);
            return 0;
        }

        try {
            const res = await axios.get(
                `https://api.coingeko.com/api/v3/simple/price`,
                {params: {ids: coinId, vs_currencies: 'usd'}}
            );

            const price = res.data[coinId]?.usd ?? 0;
            cache.set(upper, {price, expiresAt: Date.now() + CACHE_TTL_MS});
            return price;
        } catch (err) {
            console.error(`Price fetch failed for ${upper}: `, err);
            return cached?.price ?? 0;
        }
    },

    async weiToUsd(weiValue: bigint, symbol = 'ETH') : Promise<number> {
        const price = await this.getUsdPrice(symbol);
        const ethValue = Number(weiValue) / 1e18;
        return ethValue * price;
    }
};