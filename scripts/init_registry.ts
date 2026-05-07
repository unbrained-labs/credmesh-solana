// scripts/init_registry.ts — calls `init_registry` on
// credmesh-attestor-registry to allocate the singleton AttestorConfig PDA
// with the governance Squads-vault pubkey.
//
// This is a hand-rolled instruction encoder (no IDL dependency) — Anchor
// 0.30 IDL extraction is tracked separately as issue #15. Borsh field
// order MUST match `init_registry(governance: Pubkey)` in
// programs/credmesh-attestor-registry/src/lib.rs.
//
// Example:
//   npx ts-node scripts/init_registry.ts \
//     --cluster devnet \
//     --governance <SQUADS_VAULT_PUBKEY>

import { createHash } from "node:crypto";
import {
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { ATTESTOR_REGISTRY_PROGRAM_ID } from "./lib/program-ids";
import { buildProvider, parseArgs } from "./lib/cluster";

const ATTESTOR_CONFIG_SEED = Buffer.from("attestor_config");

function anchorDiscriminator(method: string): Buffer {
  return createHash("sha256").update(`global:${method}`).digest().subarray(0, 8);
}

async function main(): Promise<void> {
  const args = parseArgs(
    process.argv.slice(2),
    ["cluster", "governance"] as const,
    ["wallet"],
  );

  const provider = buildProvider({
    cluster: args.cluster,
    walletPath: args.wallet,
  });
  const deployer = provider.wallet.publicKey;
  const governance = new PublicKey(args.governance);

  if (governance.equals(PublicKey.default)) {
    throw new Error(
      "--governance must NOT be the zero pubkey. Pass a real Squads vault PDA.",
    );
  }
  if (governance.equals(deployer)) {
    throw new Error(
      "--governance must differ from the deployer wallet. Pre-create a Squads vault PDA.",
    );
  }

  const [configPda, configBump] = PublicKey.findProgramAddressSync(
    [ATTESTOR_CONFIG_SEED],
    ATTESTOR_REGISTRY_PROGRAM_ID,
  );
  console.log(`config PDA:   ${configPda.toBase58()} (bump ${configBump})`);
  console.log(`governance:   ${governance.toBase58()}`);

  const existing = await provider.connection.getAccountInfo(configPda);
  if (existing !== null) {
    console.error(
      `attestor-registry already initialised at ${configPda.toBase58()}. Refusing to re-init.`,
    );
    process.exit(2);
  }

  const data = Buffer.concat([
    anchorDiscriminator("init_registry"),
    governance.toBuffer(),
  ]);

  const ix = new TransactionInstruction({
    programId: ATTESTOR_REGISTRY_PROGRAM_ID,
    keys: [
      { pubkey: deployer, isSigner: true, isWritable: true },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });

  const tx = new Transaction().add(ix);
  const sig = await provider.sendAndConfirm(tx, [], { commitment: "confirmed" });
  console.log(`✅ attestor-registry initialised. sig: ${sig}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
