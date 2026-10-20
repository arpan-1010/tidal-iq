import Image from "next/image";

const LOGOS: Record<string, { src: string; alt: string }> = {
  ethereum: { src: "/chains/ethereum.svg", alt: "Ethereum" },
  solana: { src: "/chains/solana.svg", alt: "Solana" },
  bitcoin: { src: "/chains/bitcoin.svg", alt: "Bitcoin" },
};

export function ChainIcon({
  chain,
  size = 32,
}: {
  chain: string;
  size?: number;
}) {
  const logo = LOGOS[chain];

  if (!logo) {
    return (
      <span
        className="inline-block rounded-full bg-muted"
        style={{ width: size, height: size }}
      />
    );
  }

  return (
    <span
      className="inline-flex items-center justify-center rounded-full bg-white/95 ring-1 ring-white/10"
      style={{ width: size, height: size }}
    >
      <Image
        src={logo.src}
        alt={logo.alt}
        width={Math.round(size * 0.6)}
        height={Math.round(size * 0.6)}
        unoptimized
      />
    </span>
  );
}