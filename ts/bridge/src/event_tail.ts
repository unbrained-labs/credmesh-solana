/**
 * Solana → EVM event tail.
 *
 * Subscribes to credmesh-escrow program logs and forwards
 * AdvanceIssued / AdvanceSettled / AdvanceLiquidated deltas to the EVM
 * credit-worker so EVM AgentRecord state stays in sync with on-Solana
 * issuance.
 *
 * Today this fires HTTP POSTs against `EVM_CREDIT_WORKER_URL`. The
 * exact endpoint shape is finalised on the EVM side; we send a stable
 * `{ event, payload }` envelope and let the EVM-side worker version
 * the payload schema. If `EVM_CREDIT_WORKER_URL` is unset, the tail
 * runs but logs deltas locally instead of POSTing — useful for
 * devnet bring-up while the EVM endpoint isn't yet live.
 */

import {
  createSolanaRpcSubscriptions,
  type Address,
} from "@solana/kit";
import { anchorEventDiscriminator } from "@credmesh/solana-shared";

interface EventTailConfig {
  solanaWsUrl: string;
  escrowProgramId: Address;
  evmCreditWorkerUrl: string | null;
  agentBindings: Map<string, `0x${string}`>;
  evmCreditWorkerToken: string | null;
}

interface AdvanceEvent {
  kind: "AdvanceIssued" | "AdvanceSettled" | "AdvanceLiquidated";
  raw_b64: string;
  slot: bigint;
  signature: string;
  pool: string;
  agentSolanaPubkey: string;
  evmAddress: `0x${string}` | null;
  advance: string;
  principalAtoms?: string;
  feeOwedAtoms?: string;
  expiresAt?: string;
  attestor?: string;
  lpCutAtoms?: string;
  protocolCutAtoms?: string;
  agentNetAtoms?: string;
  lateDays?: number;
  lossAtoms?: string;
}

const EVENT_NAMES = ["AdvanceIssued", "AdvanceSettled", "AdvanceLiquidated"] as const;

export async function startEventTail(cfg: EventTailConfig): Promise<void> {
  const subs = createSolanaRpcSubscriptions(cfg.solanaWsUrl);
  const discriminators = await Promise.all(
    EVENT_NAMES.map(async (n) => ({
      name: n,
      disc: Buffer.from(await anchorEventDiscriminator(n)).toString("base64"),
    })),
  );

  console.log(`[event-tail] subscribing to escrow program logs ${cfg.escrowProgramId}`);
  console.log(
    `[event-tail] forward target: ${cfg.evmCreditWorkerUrl ?? "(none — log-only mode)"}`,
  );

  const subscription = await subs
    .logsNotifications(
      { mentions: [cfg.escrowProgramId] } as any,
      { commitment: "confirmed" } as any,
    )
    .subscribe({ abortSignal: new AbortController().signal });

  for await (const notification of subscription) {
    const value = (notification as any)?.value;
    if (!value || value.err) continue;
    const sig: string = value.signature;
    const slot: bigint = (notification as any)?.context?.slot ?? 0n;
    const logs: string[] = value.logs ?? [];
    for (const line of logs) {
      // Anchor's `emit!` writes "Program data: <base64>" lines. The first
      // 8 bytes of the decoded base64 are the event discriminator
      // (sha256("event:<EventName>")[..8]).
      const match = line.match(/^Program data: (.+)$/);
      if (!match) continue;
      const b64 = match[1];
      const decoded = Buffer.from(b64, "base64");
      if (decoded.length < 8) continue;
      const discB64 = decoded.subarray(0, 8).toString("base64");
      const evt = discriminators.find((d) => d.disc === discB64);
      if (!evt) continue;
      const payload = decodeEvent(evt.name, decoded.subarray(8));
      const evmAddress = cfg.agentBindings.get(payload.agentSolanaPubkey) ?? null;
      const event: AdvanceEvent = {
        kind: evt.name as AdvanceEvent["kind"],
        raw_b64: b64,
        slot,
        signature: sig,
        evmAddress,
        ...payload,
      };
      await forwardEvent(event, cfg);
    }
  }
}

function decodeEvent(
  kind: (typeof EVENT_NAMES)[number],
  data: Buffer,
): Omit<AdvanceEvent, "kind" | "raw_b64" | "slot" | "signature" | "evmAddress"> {
  let off = 0;
  const readPubkey = () => {
    const value = bytesToBase58(data.subarray(off, off + 32));
    off += 32;
    return value;
  };
  const readU64 = () => {
    const value = data.readBigUInt64LE(off);
    off += 8;
    return value.toString();
  };
  const readI64 = () => {
    const value = data.readBigInt64LE(off);
    off += 8;
    return value.toString();
  };
  const readU32 = () => {
    const value = data.readUInt32LE(off);
    off += 4;
    return value;
  };

  if (kind === "AdvanceIssued") {
    return {
      pool: readPubkey(),
      agentSolanaPubkey: readPubkey(),
      advance: readPubkey(),
      principalAtoms: readU64(),
      feeOwedAtoms: readU64(),
      expiresAt: readI64(),
      attestor: readPubkey(),
    };
  }
  if (kind === "AdvanceSettled") {
    return {
      pool: readPubkey(),
      agentSolanaPubkey: readPubkey(),
      advance: readPubkey(),
      principalAtoms: readU64(),
      lpCutAtoms: readU64(),
      protocolCutAtoms: readU64(),
      agentNetAtoms: readU64(),
      lateDays: readU32(),
    };
  }
  return {
    pool: readPubkey(),
    agentSolanaPubkey: readPubkey(),
    advance: readPubkey(),
    lossAtoms: readU64(),
  };
}

async function forwardEvent(event: AdvanceEvent, cfg: EventTailConfig): Promise<void> {
  if (!cfg.evmCreditWorkerUrl) {
    console.log(
      `[event-tail] ${event.kind} slot=${event.slot} sig=${event.signature.slice(0, 12)}… (log-only; EVM_CREDIT_WORKER_URL unset)`,
    );
    return;
  }
  try {
    const headers: Record<string, string> = { "Content-Type": "application/json" };
    if (cfg.evmCreditWorkerToken) {
      headers.Authorization = `Bearer ${cfg.evmCreditWorkerToken}`;
    }
    const res = await fetch(`${cfg.evmCreditWorkerUrl}/solana-event`, {
      method: "POST",
      headers,
      body: JSON.stringify({
        version: 1,
        event: event.kind,
        slot: Number(event.slot),
        signature: event.signature,
        raw_b64: event.raw_b64,
        pool: event.pool,
        agentSolanaPubkey: event.agentSolanaPubkey,
        evmAddress: event.evmAddress,
        advance: event.advance,
        principalAtoms: event.principalAtoms,
        feeOwedAtoms: event.feeOwedAtoms,
        expiresAt: event.expiresAt,
        attestor: event.attestor,
        lpCutAtoms: event.lpCutAtoms,
        protocolCutAtoms: event.protocolCutAtoms,
        agentNetAtoms: event.agentNetAtoms,
        lateDays: event.lateDays,
        lossAtoms: event.lossAtoms,
      }),
    });
    if (!res.ok) {
      console.warn(
        `[event-tail] EVM ack failed ${res.status} for ${event.kind} sig=${event.signature.slice(0, 12)}…`,
      );
    }
  } catch (err) {
    // Non-fatal: the EVM-side replay endpoint is the canonical
    // reconciliation surface; if it's down the tail keeps observing
    // and the EVM side can replay from Solana log history later.
    console.warn(`[event-tail] forward error: ${err instanceof Error ? err.message : err}`);
  }
}

function bytesToBase58(bytes: Uint8Array): string {
  const ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let zeros = 0;
  for (const b of bytes) {
    if (b === 0) zeros += 1;
    else break;
  }
  const digits = [0];
  for (let i = zeros; i < bytes.length; i++) {
    let carry = bytes[i];
    for (let j = 0; j < digits.length; j++) {
      carry += digits[j] << 8;
      digits[j] = carry % 58;
      carry = (carry / 58) | 0;
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = (carry / 58) | 0;
    }
  }
  let out = "";
  for (let i = 0; i < zeros; i++) out += "1";
  for (let i = digits.length - 1; i >= 0; i--) out += ALPHABET[digits[i]];
  return out;
}
