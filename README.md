# WZN Card Game Solana Program

## Overview

Complete Solana smart contract system for WZN Card Game implementing:

- **BurnPass Contract**: Burn WZN tokens for 30-day game access
- **GameAccess Check**: Verify user access based on burn timestamp
- **BurnVault**: Secure token storage for burned WZN
- **DAO Voting**: Governance system for platform decisions
- **Emergency Unlock**: Multi-signature emergency token recovery
- **Prize Distribution**: Automated prize distribution to winners
- **Governance Helpers**: Utility functions for voting and access checks

## Architecture

### Modules

1. **Module 1: BurnPass Contract (2.5h)**
   - `burn_for_pass()` - User burns 500-1000 WZN for 30-day access
   - `burn_for_pass_with_fee_payer()` - Project pays transaction fees
   - PDA storage for user access timestamps
   - One burn per 30-day period validation

2. **Module 2: GameAccess Check (1h)**
   - `check_access()` - Verify active access based on timestamp
   - Returns boolean for game access permission

3. **Module 3: BurnVault (1h)**
   - `initialize_burn_vault()` - Secure PDA for burned tokens
   - Admin functions for access cost and fee mode changes

4. **Module 4: DAO Voting (3h)**
   - `create_proposal()` - Create governance proposals
   - `vote_on_proposal()` - Cast votes with WZN-based voting power
   - `execute_proposal()` - Execute approved proposals
   - Quorum requirements: 100+ voters, 60%+ yes votes
   - 7-day voting period, 14-day execution window

5. **Module 5: Emergency Unlock (2h)**
   - `initiate_emergency_unlock()` - Start emergency unlock process
   - `sign_emergency_unlock()` - Collect signatures from 5 wallets
   - `execute_emergency_unlock()` - Release 15-35% of burn vault
   - 2-year time lock, 30-day cooldown between unlocks
   - Requires 5 signatures from wallets with 10,000+ WZN

6. **Module 6: Prize Distribution (2h)**
   - `submit_rewards()` - Admin submits prize distributions
   - `execute_prize_distribution()` - Transfer prizes to winners
   - Season management and distribution tracking

7. **Module 7: Governance Helpers (1.5h)**
   - `get_proposal_status()` - Check proposal state
   - `check_voting_eligibility()` - Verify user voting rights
   - `check_emergency_unlock_limits()` - Emergency unlock status
   - `get_platform_stats()` - Platform statistics

8. **Module 8: Unit Tests & Deploy (2h)**
   - Comprehensive Rust and TypeScript tests
   - Devnet deployment scripts
   - Gas/compute unit optimization

## Security Features

- **PDA-based architecture** for secure account management
- **Multi-signature emergency unlock** requires 5 wallets with 10,000+ WZN
- **Time locks and cooldowns** prevent abuse
- **Voting eligibility** requires both WZN balance and active access pass
- **Admin controls** with proper authorization checks
- **Fee delegation** support for better UX

## Business Rules

- **Burn Amount**: 500-1000 WZN per 30-day access pass
- **Voting Requirements**: 1,000+ WZN + active access for voting
- **Proposal Creation**: 10,000+ WZN + active access
- **Emergency Unlock**: 15-35% of vault, 2-year lock, 30-day cooldown
- **DAO Quorum**: 100+ voters, 60%+ approval rate
- **Fee Modes**: 0 = user pays, 1 = project pays (configurable)

## Installation & Setup

### Prerequisites

- Rust 1.70+
- Solana CLI 1.18+
- Anchor Framework 0.30+
- Node.js 18+ (for TypeScript tests)

### Build

```bash
# Build the program
cargo build-bpf

# Or using Anchor
anchor build
```

### Test

```bash
# Run Rust tests
cargo test

# Run TypeScript integration tests
npm install
npm test
```

### Deploy to Devnet

```bash
# Configure Solana CLI for devnet
solana config set --url devnet

# Get devnet SOL
solana airdrop 5

# Deploy program
anchor deploy --provider.cluster devnet

# Initialize the program
anchor run initialize --provider.cluster devnet
```

## Usage Examples

### Initialize System

```typescript
// Initialize global config
await program.methods
  .initialize(adminPublicKey)
  .accounts({
    globalConfig,
    admin: admin.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .signers([admin])
  .rpc();

// Initialize burn vault
await program.methods
  .initializeBurnVault()
  .accounts({
    burnVault,
    wznMint,
    admin: admin.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  })
  .signers([admin])
  .rpc();
```

### Burn for Access

```typescript
// User burns 500 WZN for 30-day access
await program.methods
  .burnForPass(new anchor.BN(500_000_000)) // 500 WZN
  .accounts({
    userAccess,
    globalConfig,
    burnVault,
    userTokenAccount,
    user: user.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  })
  .signers([user])
  .rpc();
```

### Check Access

```typescript
// Check if user has valid access
const hasAccess = await program.methods
  .checkAccess()
  .accounts({
    userAccess,
    user: user.publicKey,
  })
  .view();

console.log("User has access:", hasAccess);
```

### Create DAO Proposal

```typescript
// Create proposal to change access cost
await program.methods
  .createProposal(
    0, // ProposalType: ChangeAccessCost
    "Reduce access cost to 400 WZN",
    new anchor.BN(400_000_000) // 400 WZN
  )
  .accounts({
    proposal,
    globalConfig,
    userAccess,
    userWznAccount,
    wznMint,
    proposer: user.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .signers([user])
  .rpc();
```

### Vote on Proposal

```typescript
// Vote YES on proposal
await program.methods
  .voteOnProposal(proposalId, true) // true = YES vote
  .accounts({
    proposal,
    userVote,
    userAccess,
    userWznAccount,
    wznMint,
    voter: user.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .signers([user])
  .rpc();
```

## Gas/Compute Unit Limits

All functions optimized to stay within Solana limits:

- **burn_for_pass**: ~150k CU
- **vote_on_proposal**: ~120k CU
- **execute_proposal**: ~200k CU
- **emergency_unlock**: ~250k CU
- **submit_rewards**: ~300k CU (batch operations)

## Error Codes

- `InvalidBurnAmount` - Burn amount not in 500-1000 WZN range
- `AccessStillActive` - User already has active access
- `NoActiveAccess` - User needs active access pass for voting
- `InsufficientWznForVoting` - Need 1,000+ WZN to vote
- `InsufficientWznForProposal` - Need 10,000+ WZN to create proposals
- `QuorumNotMet` - Need 100+ voters for proposal execution
- `TimeLockNotExpired` - 2-year emergency lock still active
- `InsufficientSignatures` - Need 5 signatures for emergency unlock

## Security Audit Checklist

- [x] PDA seed uniqueness verified
- [x] Authorization checks on all admin functions
- [x] Overflow/underflow protection
- [x] Time-based validation (30-day periods, 2-year locks)
- [x] Multi-signature requirements enforced
- [x] Fee delegation properly implemented
- [x] Double-voting prevention
- [x] Account ownership validation
- [x] Token transfer security
- [x] Emergency procedures tested

## Future Enhancements

1. **Staking Rewards**: Additional WZN rewards for long-term stakers
2. **NFT Integration**: Special NFTs for top players
3. **Cross-chain Bridge**: Support for other blockchain networks
4. **Oracle Integration**: External data feeds for dynamic pricing
5. **Mobile SDK**: React Native components for mobile apps

## Support

For technical support or questions:
- GitHub Issues: [Repository URL]
- Discord: [Discord Server]
- Email: support@wzncard.game

---

Built with ❤️ using Solana + Anchor Framework
