import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { WznCardGame } from "../target/types/wzn_card_game";
import { 
  Keypair, 
  PublicKey, 
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getAccount,
} from "@solana/spl-token";
import { expect } from "chai";

describe("WZN Card Game", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.WznCardGame as Program<WznCardGame>;
  
  let admin: Keypair;
  let user: Keypair;
  let feePayer: Keypair;
  let wznMint: PublicKey;
  let adminTokenAccount: PublicKey;
  let userTokenAccount: PublicKey;
  let burnVault: PublicKey;
  let globalConfig: PublicKey;

  beforeEach(async () => {
    admin = Keypair.generate();
    user = Keypair.generate();
    feePayer = Keypair.generate();

    // Airdrop SOL to test accounts
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(admin.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL)
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(user.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL)
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(feePayer.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL)
    );

    // Create WZN mint
    wznMint = await createMint(
      provider.connection,
      admin,
      admin.publicKey,
      null,
      6 // 6 decimals for WZN
    );

    // Create token accounts
    adminTokenAccount = await createAccount(
      provider.connection,
      admin,
      wznMint,
      admin.publicKey
    );

    userTokenAccount = await createAccount(
      provider.connection,
      user,
      wznMint,
      user.publicKey
    );

    // Mint WZN tokens to user (100,000 WZN)
    await mintTo(
      provider.connection,
      admin,
      wznMint,
      userTokenAccount,
      admin,
      100_000_000_000 // 100,000 WZN in lamports
    );

    // Derive PDAs
    [globalConfig] = PublicKey.findProgramAddressSync(
      [Buffer.from("global_config")],
      program.programId
    );

    [burnVault] = PublicKey.findProgramAddressSync(
      [Buffer.from("burn_vault")],
      program.programId
    );
  });

  describe("Initialization", () => {
    it("Initializes the global config", async () => {
      await program.methods
        .initialize(admin.publicKey)
        .accounts({
          globalConfig,
          admin: admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();

      const config = await program.account.globalConfig.fetch(globalConfig);
      expect(config.admin.toString()).to.equal(admin.publicKey.toString());
      expect(config.accessCost.toNumber()).to.equal(500_000_000); // 500 WZN
      expect(config.feeMode).to.equal(1); // Project pays fees
    });

    it("Initializes the burn vault", async () => {
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

      const vault = await getAccount(provider.connection, burnVault);
      expect(vault.mint.toString()).to.equal(wznMint.toString());
      expect(vault.amount).to.equal(BigInt(0));
    });
  });

  describe("BurnPass Module", () => {
    beforeEach(async () => {
      // Initialize global config and burn vault
      await program.methods
        .initialize(admin.publicKey)
        .accounts({
          globalConfig,
          admin: admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();

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
    });

    it("Burns WZN for 30-day access", async () => {
      const [userAccess] = PublicKey.findProgramAddressSync(
        [Buffer.from("user_access"), user.publicKey.toBuffer()],
        program.programId
      );

      const burnAmount = 500_000_000; // 500 WZN

      await program.methods
        .burnForPass(new anchor.BN(burnAmount))
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

      const accessInfo = await program.account.userAccessInfo.fetch(userAccess);
      expect(accessInfo.user.toString()).to.equal(user.publicKey.toString());
      expect(accessInfo.amountBurned.toNumber()).to.equal(burnAmount);

      // Check that tokens were transferred to burn vault
      const vault = await getAccount(provider.connection, burnVault);
      expect(vault.amount).to.equal(BigInt(burnAmount));
    });

    it("Fails with invalid burn amount", async () => {
      const [userAccess] = PublicKey.findProgramAddressSync(
        [Buffer.from("user_access"), user.publicKey.toBuffer()],
        program.programId
      );

      try {
        await program.methods
          .burnForPass(new anchor.BN(100_000_000)) // Only 100 WZN (too low)
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
        
        expect.fail("Should have failed with invalid burn amount");
      } catch (error) {
        expect(error.message).to.include("InvalidBurnAmount");
      }
    });
  });

  describe("GameAccess Module", () => {
    beforeEach(async () => {
      // Setup and burn for access
      await program.methods
        .initialize(admin.publicKey)
        .accounts({
          globalConfig,
          admin: admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();

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
    });

    it("Checks user access correctly", async () => {
      const [userAccess] = PublicKey.findProgramAddressSync(
        [Buffer.from("user_access"), user.publicKey.toBuffer()],
        program.programId
      );

      // First burn for access
      await program.methods
        .burnForPass(new anchor.BN(500_000_000))
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

      // Then check access
      const hasAccess = await program.methods
        .checkAccess()
        .accounts({
          userAccess,
          user: user.publicKey,
        })
        .view();

      expect(hasAccess).to.be.true;
    });
  });

  describe("DAO Voting Module", () => {
    it("Creates and executes a proposal", async () => {
      // This would test the full DAO voting flow
      // 1. Create proposal
      // 2. Multiple users vote
      // 3. Check quorum
      // 4. Execute proposal
    });

    it("Prevents double voting", async () => {
      // Test that users can't vote twice on the same proposal
    });
  });

  describe("Emergency Unlock Module", () => {
    it("Executes emergency unlock with proper signatures", async () => {
      // Test emergency unlock flow
      // This would require simulating time passage for the 2-year lock
    });

    it("Prevents emergency unlock without sufficient signatures", async () => {
      // Test validation of signature requirements
    });
  });

  describe("Prize Distribution Module", () => {
    it("Distributes prizes to multiple winners", async () => {
      // Test prize distribution functionality
    });
  });

  describe("Governance Helpers", () => {
    it("Returns correct proposal status", async () => {
      // Test status checking functions
    });

    it("Returns correct voting eligibility", async () => {
      // Test eligibility checking functions
    });
  });

  describe("Gas/Compute Unit Tests", () => {
    it("BurnPass functions stay under compute limits", async () => {
      // Test that burn functions don't exceed Solana CU limits
    });

    it("DAO voting functions stay under compute limits", async () => {
      // Test that voting functions don't exceed Solana CU limits
    });

    it("Emergency unlock functions stay under compute limits", async () => {
      // Test that emergency functions don't exceed Solana CU limits
    });
  });
});
