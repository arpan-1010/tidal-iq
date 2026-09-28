export interface WhaleTransaction {
    id : string;
    hash : string;
    chain : string;
    from : string;
    to : string;
    value : string;
    valueUsd : string;
    token : string;
    tokenSymbol : string;
    blockNumber : string;
    timestamp : string;
    aiSummary : string;
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