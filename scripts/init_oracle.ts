// scripts/init_oracle.ts — calls `init_oracle` on credmesh-receivable-oracle
// to allocate the singleton OracleConfig PDA with governance + worker
// authority + per-tx/per-period caps.
//
// This script uses the IDL extracted at `target/idl/credmesh_receivable_oracle.json`
// (oracle's IDL extracted cleanly during the partial build, before escrow's
// IDL pass tripped on issue #15). Once #15 lands, regenerate the IDL via
// `anchor build` and re-run.
//
// Example:
//   npx ts-node scripts/init_oracle.ts \
//     --cluster devnet \
//     --governance <GOVERNANCE_PUBKEY> \
//     --worker-authority <HOT_WORKER_PUBKEY> \
//     --worker-max-per-tx 100000000 \
//     --worker-max-per-period 10000000000 \
//     --period-seconds 86400

import { Program, BN } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import idlJson from "../target/idl/credmesh_receivable_oracle.json";
import type { CredmeshReceivableOracle } from "../target/types/credmesh_receivable_oracle";
import { RECEIVABLE_ORACLE_PROGRAM_ID } from "./lib/program-ids";
import { buildProvider, parseArgs } from "./lib/cluster";

// `oracle_config` ASCII bytes — must match `ORACLE_CONFIG_SEED` in
// `crates/credmesh-shared::seeds`.
const ORACLE_CONFIG_SEED = Buffer.from("oracle_config");

async function main(): Promise<void> {
  const args = parseArgs(
    process.argv.slice(2),
    [
      "cluster",
      "governance",
      "worker-authority",
      "worker-max-per-tx",
      "worker-max-per-period",
      "period-seconds",
    ] as const,
    ["wallet"],
  );

  const provider = buildProvider({
    cluster: args.cluster,
    walletPath: args.wallet,
  });
  // The IDL JSON anchor generates includes `address`, which Program reads.
  // We pass the program ID explicitly anyway for safety against an IDL that
  // was extracted under a different keypair.
  const idl = { ...idlJson, address: RECEIVABLE_ORACLE_PROGRAM_ID.toBase58() };
  const program = new Program<CredmeshReceivableOracle>(
    idl as CredmeshReceivableOracle,
    provider,
  );

  const [configPda, configBump] = PublicKey.findProgramAddressSync(
    [ORACLE_CONFIG_SEED],
    RECEIVABLE_ORACLE_PROGRAM_ID,
  );
  console.log(`oracle config PDA: ${configPda.toBase58()} (bump ${configBump})`);

  // If config already exists, refuse rather than re-init (init_oracle is
  // single-shot — it uses `init`, not `init_if_needed`, so a re-init from
  // chain would error anyway, but failing here gives a clearer message).
  const existing = await provider.connection.getAccountInfo(configPda);
  if (existing !== null) {
    console.error(
      `oracle config already initialised at ${configPda.toBase58()} (${existing.data.length} bytes). Refusing to re-init.`,
    );
    process.exit(2);
  }

  const params = {
    governance: new PublicKey(args.governance),
    workerAuthority: new PublicKey(args["worker-authority"]),
    workerMaxPerTx: new BN(args["worker-max-per-tx"]),
    workerMaxPerPeriod: new BN(args["worker-max-per-period"]),
    workerPeriodSeconds: new BN(args["period-seconds"]),
  };
  console.log(`init_oracle params:`);
  console.log(`  governance:           ${params.governance.toBase58()}`);
  console.log(`  worker_authority:     ${params.workerAuthority.toBase58()}`);
  console.log(`  worker_max_per_tx:    ${params.workerMaxPerTx.toString()}`);
  console.log(`  worker_max_per_period:${params.workerMaxPerPeriod.toString()}`);
  console.log(`  worker_period_seconds:${params.workerPeriodSeconds.toString()}`);

  // 3-keys invariant (DESIGN §10): governance, worker_authority, and (later)
  // reputation_writer_authority MUST never be the same key. Worker authority
  // and governance equality is the easy one to catch here.
  if (params.governance.equals(params.workerAuthority)) {
    throw new Error(
      "governance and worker_authority MUST differ (DESIGN §10 three-key topology).",
    );
  }

  // `config` and `system_program` are auto-resolved by Anchor 0.30 from the
  // IDL's PDA seeds + the `[features] resolution = true` setting in
  // Anchor.toml — passing them explicitly is a type error.
  const sig = await program.methods
    .initOracle(params)
    .accounts({
      deployer: provider.wallet.publicKey,
    })
    .rpc({ commitment: "confirmed" });
  console.log(`\ntx: ${sig}`);
  console.log("init_oracle: ok");
}

main().catch((err) => {
  console.error("init_oracle: failed");
  console.error(err);
  process.exit(1);
});
