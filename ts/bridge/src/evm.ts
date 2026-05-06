/**
 * EVM read interface. Reads ReputationCreditOracle for an agent's
 * current credit limit and TrustlessEscrow.exposure() for outstanding
 * balance. The bridge consults these and signs the on-Solana
 * attestation; we never trust client-supplied values.
 *
 * Implemented against viem's `createPublicClient` + the EVM contract
 * ABIs. The bridge service refuses to issue attestations if any of the
 * required env vars is missing — explicit refusal beats silent
 * fallback to zero.
 */

import {
  createPublicClient,
  http,
  type Address,
  type PublicClient,
  parseAbi,
} from "viem";

const REPUTATION_CREDIT_ORACLE_ABI = parseAbi([
  "function maxExposure(address agent) view returns (uint256)",
]);

const TRUSTLESS_ESCROW_ABI = parseAbi([
  "function exposure(address agent) view returns (uint256)",
]);

export interface EvmConfig {
  rpcUrl: string;
  chainId: number;
  reputationCreditOracle: Address;
  trustlessEscrow: Address;
}

export interface EvmAgentSnapshot {
  /// In USDC atoms (6 decimals). Capped at $1000 by EVM.
  creditLimitAtoms: bigint;
  /// Current outstanding exposure across EVM-tracked advances. The EVM
  /// AgentRecord/SignerRegistry holds this; the bridge replays Solana
  /// settle/liquidate events back to keep it accurate.
  outstandingAtoms: bigint;
}

export class EvmReader {
  private client: PublicClient;

  constructor(private cfg: EvmConfig) {
    this.client = createPublicClient({
      transport: http(cfg.rpcUrl),
    });
  }

  /// Reads (creditLimitAtoms, outstandingAtoms) for an agent from EVM.
  /// Returns the values as USDC atoms (the EVM contract returns raw u256
  /// in 6-decimal USDC; we cast to bigint).
  async fetchAgent(agent: Address): Promise<EvmAgentSnapshot> {
    const [maxExposure, currentExposure] = await Promise.all([
      this.client.readContract({
        address: this.cfg.reputationCreditOracle,
        abi: REPUTATION_CREDIT_ORACLE_ABI,
        functionName: "maxExposure",
        args: [agent],
      }),
      this.client.readContract({
        address: this.cfg.trustlessEscrow,
        abi: TRUSTLESS_ESCROW_ABI,
        functionName: "exposure",
        args: [agent],
      }),
    ]);

    return {
      creditLimitAtoms: BigInt(maxExposure as bigint),
      outstandingAtoms: BigInt(currentExposure as bigint),
    };
  }
}
