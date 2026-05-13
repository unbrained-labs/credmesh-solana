// programs/credmesh-escrow/tests/helpers/memo.ts
//
// Build a Memo v2 instruction whose data is the 16-byte nonce used for
// replay-binding in `claim_and_settle` (see
// `credmesh_shared::ix_introspection::require_memo_nonce`).
//
// Memo v2 program id: MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr.
// The on-chain check compares `ix.data == expected_nonce` byte-for-byte
// (no signers required, no length prefix — raw 16 bytes).

import { PublicKey, TransactionInstruction } from "@solana/web3.js";

export const MEMO_PROGRAM_ID = new PublicKey(
  "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr",
);

export function memoNonceIx(nonce: Buffer): TransactionInstruction {
  if (nonce.length !== 16) {
    throw new Error(`memo nonce must be 16 bytes; got ${nonce.length}`);
  }
  return new TransactionInstruction({
    keys: [],
    programId: MEMO_PROGRAM_ID,
    data: nonce,
  });
}
