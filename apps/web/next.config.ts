import type { NextConfig } from "next";
import path from "path";

const nextConfig: NextConfig = {
  transpilePackages: ["@tidaliq/shared"],
  devIndicators: false,
  turbopack: {
    root: path.resolve(__dirname, "../.."),
  },
};

export default nextConfig;
