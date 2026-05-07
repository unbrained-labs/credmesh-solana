// scripts/add_allowed_signer.ts — builds the
// `add_allowed_signer(signer, kind)` instruction for
// credmesh-attestor-registry, and emits the base58 ix + accounts the
// operator drops into the Squads `vault_transaction_create` UI/CLI.
//
// The Squads multisig members then approve the proposal; once threshold
// is met, anyone can call `vault_transaction_execute` and our handler's
// `require_squads_governance_cpi` gate will pass (the executing tx
// contains a Squads ix authorizing-and-spending the governance vault
// against this exact inner ix).
//
// We do NOT submit the Squads tx ourselves — that requires the
// multisig members' signatures, which live with the people, not this
// script. This script's job is to produce a copy-pasteable tx
// description.
//
// Example:
//   npx ts-node scripts/add_allowed_signer.ts \
//     --cluster devnet \
//     --signer <BRIDGE_ED25519_PUBKEY> \
//     --kind 0
//
// Kind constants (mirror crates/credmesh-shared/src/lib.rs::AttestorKind):
//   0 = CreditBridge  (the EVM-attestation bridge ed25519 signer)

import { createHash } from "node:crypto";
import {
  PublicKey,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  TransactionInstruction,
} from "@solana/web3.js";
import { ATTESTOR_REGISTRY_PROGRAM_ID } from "./lib/program-ids";
import { parseArgs } from "./lib/cluster";

const ATTESTOR_CONFIG_SEED = Buffer.from("attestor_config");
const ALLOWED_SIGNER_SEED = Buffer.from("allowed_signer");

function anchorDiscriminator(method: string): Buffer {
  return createHash("sha256").update(`global:${method}`).digest().subarray(0, 8);
}

function describeAccount(name: string, key: PublicKey, signer: boolean, writable: boolean): string {
  return `  ${name.padEnd(22)}  ${key.toBase58()}  ${signer ? "S " : "  "}${writable ? "W " : "  "}`;
}

async function main(): Promise<void> {
  const args = parseArgs(
    process.argv.slice(2),
    ["cluster", "signer", "kind"] as const,
    ["cranker"],
  );

  const signer = new PublicKey(args.signer);
  const kind = parseInt(args.kind, 10);
  if (!(kind === 0)) {
    throw new Error(
      `--kind must be 0 (CreditBridge). Future kinds will be added in lockstep with crates/credmesh-shared::AttestorKind.`,
    );
  }
  if (signer.equals(PublicKey.default)) {
    throw new Error("--signer must NOT be the zero pubkey");
  }

  // The cranker pays rent for the new AllowedSigner PDA. It can be ANY
  // signer in the executing tx — typically a relayer or whoever fires
  // `vault_transaction_execute`. Default to the SystemProgram address as
  // a placeholder the operator overrides; warn loudly.
  const cranker = args.cranker
    ? new PublicKey(args.cranker)
    : SystemProgram.programId;
  if (cranker.equals(SystemProgram.programId)) {
    console.warn(
      "WARNING: --cranker not specified. The Squads tx executor pays rent for the new AllowedSigner PDA; specify --cranker explicitly when submitting via the Squads UI.",
    );
  }

  const [configPda] = PublicKey.findProgramAddressSync(
    [ATTESTOR_CONFIG_SEED],
    ATTESTOR_REGISTRY_PROGRAM_ID,
  );
  const [allowedSignerPda, allowedBump] = PublicKey.findProgramAddressSync(
    [ALLOWED_SIGNER_SEED, signer.toBuffer()],
    ATTESTOR_REGISTRY_PROGRAM_ID,
  );

  const data = Buffer.concat([
    anchorDiscriminator("add_allowed_signer"),
    signer.toBuffer(),
    Buffer.from([kind]),
  ]);

  const ix = new TransactionInstruction({
    programId: ATTESTOR_REGISTRY_PROGRAM_ID,
    keys: [
      { pubkey: cranker, isSigner: true, isWritable: true },
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: allowedSignerPda, isSigner: false, isWritable: true },
      { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });

  console.log("─────────────────────────────────────────────────────────────");
  console.log("Squads vault_transaction_create payload");
  console.log("─────────────────────────────────────────────────────────────");
  console.log(`Program ID:   ${ATTESTOR_REGISTRY_PROGRAM_ID.toBase58()}`);
  console.log(`Cluster:      ${args.cluster}`);
  console.log("Inner ix accounts:");
  console.log(describeAccount("cranker (fee payer)", cranker, true, true));
  console.log(describeAccount("config (read)", configPda, false, false));
  console.log(describeAccount("allowed_signer (init)", allowedSignerPda, false, true));
  console.log(describeAccount("instructions sysvar", SYSVAR_INSTRUCTIONS_PUBKEY, false, false));
  console.log(describeAccount("system program", SystemProgram.programId, false, false));
  console.log(`\nInstruction data (hex):`);
  console.log(`  ${ix.data.toString("hex")}`);
  console.log(`\nDerived AllowedSigner PDA:`);
  console.log(`  ${allowedSignerPda.toBase58()}  (bump ${allowedBump})`);
  console.log(`\nNew bridge signer:`);
  console.log(`  pubkey:  ${signer.toBase58()}`);
  console.log(`  kind:    ${kind} (CreditBridge)`);
  console.log("─────────────────────────────────────────────────────────────");
  console.log(
    "Drop the above into the Squads vault_transaction_create flow.\n" +
      "Once the multisig threshold approves, ANY signer can call\n" +
      "vault_transaction_execute — the on-chain require_squads_governance_cpi\n" +
      "gate will accept (the executing tx contains a Squads ix authorizing\n" +
      "the governance vault as writable).",
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
