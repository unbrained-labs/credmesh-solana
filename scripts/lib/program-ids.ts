import { PublicKey } from "@solana/web3.js";

// Devnet program IDs. Single source of truth for the deploy + init scripts;
// must stay in sync with `Anchor.toml`, each program's `declare_id!`, and
// `crates/credmesh-shared::program_ids::*`.
export const ESCROW_PROGRAM_ID = new PublicKey(
  "DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF",
);
export const ATTESTOR_REGISTRY_PROGRAM_ID = new PublicKey(
  "ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk",
);

// External program ID. Squads governs `Pool.governance` and the
// attestor-registry's `AttestorConfig.governance`.
export const SQUADS_V4 = new PublicKey(
  "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf",
);

// Devnet USDC (Circle). Mainnet USDC is `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`.
export const USDC_MINT_DEVNET = new PublicKey(
  "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU",
);
