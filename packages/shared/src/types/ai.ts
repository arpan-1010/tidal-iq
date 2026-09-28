export interface AISummary {
    transactionId : string;
    summary : string;
    impactScore : number;
    isANomaly : boolean;
    anomalyReason ?: string;
    citations : string[];
    generatedAt : number;
}

export interface ChatMessage {
    role : 'user | assistant';
    content : string;
    timestamp : number;
}