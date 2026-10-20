import Image from "next/image";

export function BrandLogo() {
  return (
    <a
      href="#top"
      className="pointer-events-auto inline-flex items-center gap-5"
    >
      <span className="relative block h-16 w-16 shrink-0 overflow-hidden rounded-2xl bg-[#010b26] shadow-md">
        <Image
          src="/brand/tidaliq.png"
          alt="TidalIQ logo"
          width={1254}
          height={1254}
          sizes="100px"
          priority
          className="absolute max-w-none"
          style={{
            width: "153%",
            height: "auto",
            left: "-27.4%",
            top: "-26.6%",
          }}
        />
      </span>

      <span className="text-4xl font-extrabold leading-none tracking-normal">
        <span className="text-foreground">Tidal</span>
        <span className="text-sky-600">IQ</span>
      </span>
    </a>
  );
}