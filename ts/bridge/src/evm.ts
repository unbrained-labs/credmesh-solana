/**
 * EVM read interface. Reads ReputationCreditOracle for an agent's
 * current credit limit and EVM-lane outstanding balance. The bridge consults
 * these and signs the on-Solana
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
  "function getCredit(address agent) view returns (uint256 score, uint256 totalExposure, uint256 maxExposure)",
  "function maxExposure(address agent) view returns (uint256)",
]);

const TRUSTLESS_ESCROW_ABI = parseAbi([
  "function exposure(address agent) view returns (uint256)",
]);

export interface EvmConfig {
  rpcUrl: string;
  reputationCreditOracle: Address;
  trustlessEscrow: Address;
}

export interface EvmAgentSnapshot {
  /// In USDC atoms (6 decimals). Capped at $1000 by EVM.
  creditLimitAtoms: bigint;
  /// Current outstanding exposure from the EVM lane only. Solana adds its
  /// local live_principal on-chain; including replayed Solana exposure here
  /// would double-count it.
  outstandingAtoms: bigint;
}

export class EvmReader {
  private client: PublicClient;

  constructor(private cfg: EvmConfig) {
    this.client = createPublicClient({
      transport: http(cfg.rpcUrl),
    });
  }

  /// Reads (creditLimitAtoms, evmOutstandingAtoms) for an agent from EVM.
  /// Returns the values as USDC atoms (the EVM contract returns raw u256
  /// in 6-decimal USDC; we cast to bigint).
  async fetchAgent(agent: Address): Promise<EvmAgentSnapshot> {
    const currentExposure = await this.client.readContract({
      address: this.cfg.trustlessEscrow,
      abi: TRUSTLESS_ESCROW_ABI,
      functionName: "exposure",
      args: [agent],
    });

    try {
      const [, , maxExposure] = await this.client.readContract({
        address: this.cfg.reputationCreditOracle,
        abi: REPUTATION_CREDIT_ORACLE_ABI,
        functionName: "getCredit",
        args: [agent],
      });
      return {
        creditLimitAtoms: maxExposure,
        outstandingAtoms: currentExposure,
      };
    } catch (err) {
      console.warn(
        `[evm] getCredit failed; falling back to legacy maxExposure read: ${err instanceof Error ? err.message : err}`,
      );
    }

    const maxExposure = await this.client.readContract({
      address: this.cfg.reputationCreditOracle,
      abi: REPUTATION_CREDIT_ORACLE_ABI,
      functionName: "maxExposure",
      args: [agent],
    });

    return {
      creditLimitAtoms: maxExposure,
      outstandingAtoms: currentExposure,
    };
  }
}
