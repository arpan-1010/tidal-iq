import { createPublicClient, http, parseAbiItem, formatUnits, type Log } from "viem";
import { mainnet } from "viem/chains";
import { priceService } from "./price.service";
import { transactionRepo } from "../db/repositories/txn.repo";
import type { WhaleTransaction } from "@whale/shared";

const client = createPublicClient({
    chain: mainnet,
    transport: http(
        `http://eth-mainnet.g.alchemy.com/v2/${process.env.ALCHEMY_API_KEY}`
    ),
});

const TRANSFER_EVENT = parseAbiItem(
    'event Transfer(address indexed from, address indexed to, uint256 value)'
);

const THRESHOLD_USD = parseFloat(process.env.WHALE_THRESHOLD_USD ?? '500000');

const WATCHED_TOKENS: Record<string, {symbol: string, decimals: number}> = {
    '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48': {symbol: 'USDC', decimals: 6},
    '0xdAC17F958D2ee523a2206206994597C13D831ec7': {symbol: 'USDT', decimals: 6},
    '0x6B175474E89094C44Da98b954EedeAC495271d0F': {symbol: 'DAI', decimals: 18},
    '0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599': {symbol: 'WBTC', decimals: 8},
};

type TransactionHandler = (tx: WhaleTransaction) => void;
const handlers: TransactionHandler[] = [];

export const blockChainServices = {
    onWhaleDetected(handler: TransactionHandler) {
        handlers.push(handler);
    },

    async watchEthTransfers() {
        console.log('Watching ETH transfers...');
        client.watchBlocks({
            onBlock: async (block) => {
                if(!block.transactions) return;

                for(const txHash of block.transactions) {
                    try {
                        const tx = await client.getTransaction({hash: txHash as `0x${string}`});
                        if(!tx || tx.value === 0n) continue;

                        const valueUsd = await priceService.weiToUsd(tx.value, 'ETH');
                        if(valueUsd < THRESHOLD_USD) continue;

                        console.log(`Large ETH transfer detected: ${valueUsd.toLocaleString}`);

                        const whaleTx: Omit<WhaleTransaction, 'id'> = {
                            hash: tx.hash,
                            chain: 'ethereum',
                            from: tx.from,
                            to: tx.to ?? '0x0000000000000000000000000000000000000000',
                            value: tx.value.toString(),
                            valueUsd: valueUsd as number,
                            token: '0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE',
                            tokenSymbol: 'ETH',
                            blockNumber: Number(block.number),
                            timestamp: Number(block.timestamp),
                        };

                        await this.saveAndNotify(whaleTx);

                    } catch (err) {
                        console.error('Error processing ETH tx: ', err);
                    }
                }
            },
        });
    },

    async watchERC20Transfers() {
        console.log('Watching ERC-20 transfers...');
        
        client.watchEvent({
            onLogs: async (logs) => {
                for(const log of logs) {
                    try {
                        const tokenAddress = Object.keys(WATCHED_TOKENS).find(
                            (k) => k.toLowerCase() === log.address.toLowerCase()
                        );
                        if(!tokenAddress) continue;

                        const tokenInfo = WATCHED_TOKENS[tokenAddress];

                        const {args} = log as any;
                        if(!args?.value) continue;

                        const valueUsd = await priceService.weiToUsd(args.value, tokenInfo.symbol);
                        if(valueUsd < THRESHOLD_USD) continue;

                        console.log(`Large ${tokenInfo.symbol} transfer: $${valueUsd.toLocaleString()}`);
                        
                        const block = await client.getBlock({blockHash: log.blockHash!});

                        const whaleTx: Omit<WhaleTransaction, 'id'> = {
                            hash: log.transactionHash!,
                            chain: 'ethereum',
                            from: args.from,
                            to: args.to,
                            value: args.value.toString(),
                            valueUsd,
                            token: log.address,
                            tokenSymbol: tokenInfo.symbol,
                            blockNumber: Number(log.blockNumber),
                            timestamp: Number(block.timestamp),
                        };

                        await this.saveAndNotify(whaleTx);

                    } catch (err) {
                        console.error('Error processing ERC-20 log: ', err);
                    }
                }
            },
        });
    },

    async saveAndNotify(tx: Omit<WhaleTransaction, 'id'>) {
        const existing = await transactionRepo.getByHash(tx.hash);
        if(existing) return;

        const id = await transactionRepo.create(tx);
        const fullTx: WhaleTransaction = {id, ...tx};

        for(const handler of handlers) {
            handler(fullTx);
        }
    },

    async start() {
        console.log(`Blockchain service starting (threshold: ${THRESHOLD_USD.toString()})`);
        await Promise.all([
            this.watchEthTransfers(),
            this.watchERC20Transfers(),
        ]);
    },
};