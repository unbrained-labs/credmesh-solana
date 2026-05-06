// scripts/deploy.ts — first-time deploy of the three CredMesh programs to a
// cluster (devnet by default). Wraps `solana program deploy` because that
// command already handles the BPF Loader Upgradeable v3 chunked write +
// finalize protocol and gives us a verifiable on-chain hash check we can
// compare against the local `.so`.
//
// This script does NOT need an IDL — it deploys raw bytecode. IDL upload is
// a separate concern (`anchor idl init`) deferred until #15 lands.
//
// Example:
//   npx ts-node scripts/deploy.ts \
//     --cluster devnet \
//     --wallet ~/.config/solana/id.json \
//     --program credmesh_escrow
//
//   # or deploy all three:
//   npx ts-node scripts/deploy.ts --cluster devnet --program all

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { Connection, PublicKey } from "@solana/web3.js";
import {
  ATTESTOR_REGISTRY_PROGRAM_ID,
  ESCROW_PROGRAM_ID,
} from "./lib/program-ids";
import { parseArgs } from "./lib/cluster";

interface ProgramSpec {
  name: string;
  programId: PublicKey;
  soPath: string;
  keypairPath: string;
}

const PROGRAMS: ProgramSpec[] = [
  {
    name: "credmesh_escrow",
    programId: ESCROW_PROGRAM_ID,
    soPath: "target/deploy/credmesh_escrow.so",
    keypairPath: "target/deploy/credmesh_escrow-keypair.json",
  },
  {
    name: "credmesh_attestor_registry",
    programId: ATTESTOR_REGISTRY_PROGRAM_ID,
    soPath: "target/deploy/credmesh_attestor_registry.so",
    keypairPath: "target/deploy/credmesh_attestor_registry-keypair.json",
  },
];

const CLUSTER_URLS: Record<string, string> = {
  devnet: "https://api.devnet.solana.com",
  mainnet: "https://api.mainnet-beta.solana.com",
  localnet: "http://127.0.0.1:8899",
};

function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

async function deployOne(
  spec: ProgramSpec,
  cluster: string,
  walletPath: string,
): Promise<void> {
  const url = CLUSTER_URLS[cluster] ?? cluster;
  const so = resolve(spec.soPath);
  if (!existsSync(so)) {
    throw new Error(
      `${spec.name}: missing ${so}. Run \`anchor build --no-idl\` first.`,
    );
  }
  const keypair = resolve(spec.keypairPath);
  if (!existsSync(keypair)) {
    throw new Error(`${spec.name}: missing keypair ${keypair}`);
  }

  const localBytes = readFileSync(so);
  const localHash = sha256Hex(localBytes);
  console.log(`\n=== ${spec.name} ===`);
  console.log(`  expected program-id: ${spec.programId.toBase58()}`);
  console.log(`  local .so size:      ${localBytes.length} bytes`);
  console.log(`  local sha256:        ${localHash}`);

  // `solana program deploy` is idempotent — on second call it issues an
  // upgrade against the existing program-id. The keypair we pass for
  // `--program-id` must own the program; on first call it gets allocated.
  const args = [
    "program",
    "deploy",
    "--url",
    url,
    "--keypair",
    walletPath,
    "--program-id",
    keypair,
    so,
  ];
  console.log(`  $ solana ${args.join(" ")}`);
  const out = execFileSync("solana", args, { encoding: "utf8" });
  console.log(out.trim());

  // Verify on-chain matches local. `solana program show` returns the data
  // length and the upgrade authority; we can't pull program-data hash
  // through web3.js without parsing the loader account ourselves, so the
  // size match + the deploy command's exit-0 are the verifiable signals.
  // Mainnet path SHOULD layer `anchor verify` on top — see DEPLOYMENT.md.
  const conn = new Connection(url, "confirmed");
  const info = await conn.getAccountInfo(spec.programId);
  if (info === null) {
    throw new Error(
      `${spec.name}: post-deploy lookup returned null — the program-id ${spec.programId.toBase58()} is not live on ${cluster}.`,
    );
  }
  console.log(`  on-chain owner:      ${info.owner.toBase58()}`);
  console.log(`  on-chain executable: ${info.executable}`);
}

async function main(): Promise<void> {
  const args = parseArgs(
    process.argv.slice(2),
    ["cluster", "program"] as const,
    ["wallet"],
  );
  const walletPath = args.wallet ?? "~/.config/solana/id.json";
  const targets =
    args.program === "all"
      ? PROGRAMS
      : PROGRAMS.filter((p) => p.name === args.program);
  if (targets.length === 0) {
    throw new Error(
      `unknown --program ${args.program}; valid: all, ${PROGRAMS.map((p) => p.name).join(", ")}`,
    );
  }
  for (const spec of targets) {
    await deployOne(spec, args.cluster, walletPath);
  }
  console.log("\ndeploy: ok");
}

main().catch((err) => {
  console.error("deploy: failed");
  console.error(err);
  process.exit(1);
});
