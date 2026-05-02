import { AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair } from "@solana/web3.js";
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";

const CLUSTER_URLS: Record<string, string> = {
  devnet: "https://api.devnet.solana.com",
  testnet: "https://api.testnet.solana.com",
  mainnet: "https://api.mainnet-beta.solana.com",
  localnet: "http://127.0.0.1:8899",
};

export interface ClusterOpts {
  cluster: string;
  walletPath?: string;
}

export function loadKeypair(path: string): Keypair {
  const expanded = path.startsWith("~")
    ? resolve(homedir(), path.slice(2))
    : resolve(path);
  const raw = JSON.parse(readFileSync(expanded, "utf8")) as number[];
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

export function buildProvider(opts: ClusterOpts): AnchorProvider {
  const url = CLUSTER_URLS[opts.cluster] ?? opts.cluster; // accept raw URLs too
  const walletPath = opts.walletPath ?? "~/.config/solana/id.json";
  const wallet = new Wallet(loadKeypair(walletPath));
  const connection = new Connection(url, "confirmed");
  return new AnchorProvider(connection, wallet, {
    preflightCommitment: "confirmed",
    commitment: "confirmed",
  });
}

// Tiny argv parser: `--key value` and `--flag` (boolean). Exits with usage on
// missing required keys to keep the deploy/init scripts self-documenting.
export function parseArgs<K extends string>(
  argv: string[],
  required: readonly K[],
  optional: readonly string[] = [],
): Record<K, string> & Record<string, string | undefined> {
  const out: Record<string, string | undefined> = {};
  for (let i = 0; i < argv.length; i++) {
    const tok = argv[i];
    if (!tok.startsWith("--")) continue;
    const key = tok.slice(2);
    const next = argv[i + 1];
    if (next === undefined || next.startsWith("--")) {
      out[key] = "true";
    } else {
      out[key] = next;
      i++;
    }
  }
  const missing = required.filter((k) => out[k] === undefined);
  if (missing.length > 0) {
    const all = [...required, ...optional];
    const usage = all
      .map((k) => `  --${k} <value>${(required as readonly string[]).includes(k) ? " (required)" : ""}`)
      .join("\n");
    console.error(`missing required args: ${missing.join(", ")}\n\nusage:\n${usage}`);
    process.exit(1);
  }
  return out as Record<K, string> & Record<string, string | undefined>;
}
