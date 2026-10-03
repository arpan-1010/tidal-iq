export interface WhaleTransaction {
    id : string;
    hash : string;
    chain : string;
    from : string;
    to : string;
    value : string;
    valueUsd : number;
    token : string;
    tokenSymbol : string;
    blockNumber : number;
    timestamp : number;
    aiSummary ?: string;
    impactScore ?: number;
    isAnomaly ?: boolean;
}

export interface WalletProfile {
    address : string;
    label ?: string;
    totalVolumeUsd : number;
    transactionCount : number;
    lastSeen : number;   
}