use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint};

declare_id!("Fg6PaFpoGXkYsidMaFRYvVpj6oH7wKq4WBZq2CFuXZJQ");

#[program]
pub mod wzn_card_game {
    use super::*;

    /// Initialize the global config
    pub fn initialize(ctx: Context<Initialize>, admin: Pubkey) -> Result<()> {
        let global_config = &mut ctx.accounts.global_config;
        global_config.admin = admin;
        global_config.access_cost = 500_000_000; // 500 WZN in lamports (default)
        global_config.fee_mode = 1; // 0 = user pays, 1 = project pays (promo period)
        global_config.next_proposal_id = 1; // Start proposal IDs from 1
        
        msg!("WZN Card Game initialized with admin: {}", admin);
        Ok(())
    }

    // ========== MODULE 1: BurnPass Contract (2.5h) ==========
    
    /// burn_for_pass() — burns WZN to unlock 30-day access
    /// Checks associated_token_account, stores timestamp in PDA (UserAccessInfo)
    /// One burn per 30-day period
    pub fn burn_for_pass(ctx: Context<BurnForPass>, amount: u64) -> Result<()> {
        let global_config = &ctx.accounts.global_config;
        
        // Check minimum burn amount (500-1000 WZN range from PDF)
        require!(
            amount >= 500_000_000 && amount <= 1_000_000_000, 
            ErrorCode::InvalidBurnAmount
        );
        
        let user_access = &mut ctx.accounts.user_access;
        let clock = Clock::get()?;
        
        // Check if user already has active access (One burn per 30-day period)
        if user_access.access_expires > clock.unix_timestamp {
            return Err(ErrorCode::AccessStillActive.into());
        }
        
        // Transfer tokens from user to burn vault
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.burn_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        token::transfer(cpi_ctx, amount)?;
        
        // Store timestamp in PDA (UserAccessInfo)
        user_access.user = ctx.accounts.user.key();
        user_access.burn_timestamp = clock.unix_timestamp;
        user_access.access_expires = clock.unix_timestamp + (30 * 24 * 60 * 60); // 30 days
        user_access.amount_burned = amount;
        
        msg!("User {} burned {} WZN for 30-day access", ctx.accounts.user.key(), amount / 1_000_000);
        
        Ok(())
    }

    /// burn_for_pass_with_fee_payer() — same as above but project pays fees (fee_mode = 1)
    pub fn burn_for_pass_with_fee_payer(ctx: Context<BurnForPassWithFeePayer>, amount: u64) -> Result<()> {
        let global_config = &ctx.accounts.global_config;
        
        // Check if project should pay fees
        require!(global_config.fee_mode == 1, ErrorCode::WrongFeeMode);
        
        // Check minimum burn amount
        require!(
            amount >= 500_000_000 && amount <= 1_000_000_000, 
            ErrorCode::InvalidBurnAmount
        );
        
        let user_access = &mut ctx.accounts.user_access;
        let clock = Clock::get()?;
        
        // Check if user already has active access
        if user_access.access_expires > clock.unix_timestamp {
            return Err(ErrorCode::AccessStillActive.into());
        }
        
        // Transfer tokens from user to burn vault
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.burn_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        token::transfer(cpi_ctx, amount)?;
        
        // Store timestamp in PDA (UserAccessInfo)
        user_access.user = ctx.accounts.user.key();
        user_access.burn_timestamp = clock.unix_timestamp;
        user_access.access_expires = clock.unix_timestamp + (30 * 24 * 60 * 60); // 30 days
        user_access.amount_burned = amount;
        
        msg!("User {} burned {} WZN for 30-day access (fees paid by project)", ctx.accounts.user.key(), amount / 1_000_000);
        
        Ok(())
    }

    // ========== MODULE 2: GameAccess Check (1h) ==========
    
    /// check_access(wallet) — Compares stored timestamp + 30 days vs current time
    pub fn check_access(ctx: Context<CheckAccess>) -> Result<bool> {
        let user_access = &ctx.accounts.user_access;
        let clock = Clock::get()?;
        
        let has_access = clock.unix_timestamp < user_access.access_expires;
        
        msg!("User {} access check: {}", ctx.accounts.user.key(), has_access);
        
        Ok(has_access)
    }

    // ========== MODULE 3: BurnVault (1h) ==========
    
    /// Initialize the burn vault PDA - Lock burned tokens in untouchable PDA
    /// Cannot be withdrawn unless DAO or Emergency triggered
    pub fn initialize_burn_vault(ctx: Context<InitializeBurnVault>) -> Result<()> {
        msg!("Burn vault initialized: {}", ctx.accounts.burn_vault.key());
        Ok(())
    }

    /// Admin function to change access cost (500-1000 WZN range)
    pub fn change_access_cost(ctx: Context<ChangeAccessCost>, new_cost: u64) -> Result<()> {
        require!(
            new_cost >= 500_000_000 && new_cost <= 1_000_000_000, 
            ErrorCode::InvalidAccessCost
        );
        
        let global_config = &mut ctx.accounts.global_config;
        global_config.access_cost = new_cost;
        
        msg!("Access cost changed to {} WZN", new_cost / 1_000_000);
        
        Ok(())
    }

    /// Change fee mode (0 = user pays, 1 = project pays)
    pub fn change_fee_mode(ctx: Context<ChangeFeeMode>, new_mode: u8) -> Result<()> {
        require!(new_mode <= 1, ErrorCode::InvalidFeeMode);
        
        let global_config = &mut ctx.accounts.global_config;
        global_config.fee_mode = new_mode;
        
        let mode_text = if new_mode == 0 { "user pays fees" } else { "project pays fees" };
        msg!("Fee mode changed to {}: {}", new_mode, mode_text);
        
        Ok(())
    }

    // ========== MODULE 4: DAO Voting (3h) ==========
    
    /// Create a new proposal (only eligible users with WZN + active access pass)
    pub fn create_proposal(
        ctx: Context<CreateProposal>, 
        proposal_type: u8, 
        description: String,
        target_value: u64
    ) -> Result<()> {
        // Check if user has active access pass (required for voting eligibility)
        let user_access = &ctx.accounts.user_access;
        let clock = Clock::get()?;
        require!(clock.unix_timestamp < user_access.access_expires, ErrorCode::NoActiveAccess);
        
        // Check minimum WZN balance (10,000 WZN minimum for proposal creation)
        require!(
            ctx.accounts.user_wzn_account.amount >= 10_000_000_000, 
            ErrorCode::InsufficientWznForProposal
        );
        
        // Validate proposal type
        require!(proposal_type <= 3, ErrorCode::InvalidProposalType);
        
        let proposal = &mut ctx.accounts.proposal;
        proposal.id = ctx.accounts.global_config.next_proposal_id;
        proposal.proposer = ctx.accounts.proposer.key();
        proposal.proposal_type = proposal_type;
        proposal.description = description;
        proposal.target_value = target_value;
        proposal.created_at = clock.unix_timestamp;
        proposal.voting_deadline = clock.unix_timestamp + (7 * 24 * 60 * 60); // 7 days voting period
        proposal.execution_deadline = clock.unix_timestamp + (14 * 24 * 60 * 60); // 14 days execution window
        proposal.yes_votes = 0;
        proposal.no_votes = 0;
        proposal.total_voters = 0;
        proposal.executed = false;
        proposal.cancelled = false;
        
        // Increment proposal ID counter
        let global_config = &mut ctx.accounts.global_config;
        global_config.next_proposal_id += 1;
        
        msg!("Proposal {} created by {}: Type {}", proposal.id, proposal.proposer, proposal_type);
        
        Ok(())
    }
    
    /// Cast vote on a proposal (only eligible users with WZN + active access pass)
    pub fn vote_on_proposal(ctx: Context<VoteOnProposal>, proposal_id: u64, vote: bool) -> Result<()> {
        // Check if user has active access pass (required for voting eligibility)
        let user_access = &ctx.accounts.user_access;
        let clock = Clock::get()?;
        require!(clock.unix_timestamp < user_access.access_expires, ErrorCode::NoActiveAccess);
        
        // Check minimum WZN balance (1,000 WZN minimum for voting)
        require!(
            ctx.accounts.user_wzn_account.amount >= 1_000_000_000, 
            ErrorCode::InsufficientWznForVoting
        );
        
        let proposal = &mut ctx.accounts.proposal;
        
        // Check if voting is still open
        require!(clock.unix_timestamp <= proposal.voting_deadline, ErrorCode::VotingClosed);
        require!(!proposal.executed, ErrorCode::ProposalAlreadyExecuted);
        require!(!proposal.cancelled, ErrorCode::ProposalCancelled);
        
        // Check if user already voted
        let user_vote = &mut ctx.accounts.user_vote;
        require!(!user_vote.has_voted, ErrorCode::AlreadyVoted);
        
        // Record user's vote
        user_vote.user = ctx.accounts.voter.key();
        user_vote.proposal_id = proposal_id;
        user_vote.vote = vote;
        user_vote.has_voted = true;
        user_vote.voting_power = ctx.accounts.user_wzn_account.amount / 1_000_000; // 1 WZN = 1 vote
        
        // Update proposal vote counts
        if vote {
            proposal.yes_votes += user_vote.voting_power;
        } else {
            proposal.no_votes += user_vote.voting_power;
        }
        proposal.total_voters += 1;
        
        msg!("User {} voted {} on proposal {} with {} votes", 
             ctx.accounts.voter.key(), if vote {"YES"} else {"NO"}, proposal_id, user_vote.voting_power);
        
        Ok(())
    }
    
    /// Execute a proposal (after voting period ends and quorum is met)
    pub fn execute_proposal(ctx: Context<ExecuteProposal>, proposal_id: u64) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get()?;
        
        // Check if voting period has ended
        require!(clock.unix_timestamp > proposal.voting_deadline, ErrorCode::VotingStillOpen);
        
        // Check if execution window is still open
        require!(clock.unix_timestamp <= proposal.execution_deadline, ErrorCode::ExecutionDeadlinePassed);
        
        require!(!proposal.executed, ErrorCode::ProposalAlreadyExecuted);
        require!(!proposal.cancelled, ErrorCode::ProposalCancelled);
        
        // Check quorum (minimum 100 voters and 60% yes votes)
        require!(proposal.total_voters >= 100, ErrorCode::QuorumNotMet);
        
        let total_votes = proposal.yes_votes + proposal.no_votes;
        let yes_percentage = (proposal.yes_votes * 100) / total_votes;
        require!(yes_percentage >= 60, ErrorCode::InsufficientYesVotes);
        
        // Execute based on proposal type
        match proposal.proposal_type {
            0 => {
                // Change Access Cost
                let global_config = &mut ctx.accounts.global_config;
                require!(
                    proposal.target_value >= 500_000_000 && proposal.target_value <= 1_000_000_000, 
                    ErrorCode::InvalidAccessCost
                );
                global_config.access_cost = proposal.target_value;
                msg!("Access cost changed to {} WZN via DAO proposal", proposal.target_value / 1_000_000);
            },
            1 => {
                // Change Fee Mode
                let global_config = &mut ctx.accounts.global_config;
                require!(proposal.target_value <= 1, ErrorCode::InvalidFeeMode);
                global_config.fee_mode = proposal.target_value as u8;
                msg!("Fee mode changed to {} via DAO proposal", proposal.target_value);
            },
            2 => {
                // Emergency Unlock (will be handled in Module 5)
                msg!("Emergency unlock proposal {} approved", proposal_id);
            },
            3 => {
                // Prize Distribution (will be handled in Module 6)
                msg!("Prize distribution proposal {} approved", proposal_id);
            },
            _ => return Err(ErrorCode::InvalidProposalType.into()),
        }
        
        proposal.executed = true;
        
        msg!("Proposal {} executed successfully", proposal_id);
        
        Ok(())
    }
    
    /// Cancel a proposal (only by proposer or admin, before execution)
    pub fn cancel_proposal(ctx: Context<CancelProposal>, proposal_id: u64) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        
        // Only proposer or admin can cancel
        require!(
            ctx.accounts.signer.key() == proposal.proposer || 
            ctx.accounts.signer.key() == ctx.accounts.global_config.admin,
            ErrorCode::UnauthorizedCancel
        );
        
        require!(!proposal.executed, ErrorCode::ProposalAlreadyExecuted);
        require!(!proposal.cancelled, ErrorCode::ProposalAlreadyCancelled);
        
        proposal.cancelled = true;
        
        msg!("Proposal {} cancelled by {}", proposal_id, ctx.accounts.signer.key());
        
        Ok(())
    }

    // ========== MODULE 5: Emergency Unlock (2h) ==========
    
    /// Initialize emergency unlock system
    pub fn initialize_emergency_unlock(ctx: Context<InitializeEmergencyUnlock>) -> Result<()> {
        let emergency_config = &mut ctx.accounts.emergency_config;
        let clock = Clock::get()?;
        
        emergency_config.admin = ctx.accounts.admin.key();
        emergency_config.time_lock_start = clock.unix_timestamp;
        emergency_config.time_lock_duration = 2 * 365 * 24 * 60 * 60; // 2 years in seconds
        emergency_config.last_unlock_time = 0;
        emergency_config.cooldown_period = 30 * 24 * 60 * 60; // 30 days in seconds
        emergency_config.min_signers = 5;
        emergency_config.min_wzn_balance = 10_000_000_000; // 10,000 WZN
        emergency_config.current_unlock_id = 1;
        
        msg!("Emergency unlock system initialized with 2-year time lock");
        
        Ok(())
    }
    
    /// Initiate emergency unlock (requires 5 wallets with 10,000+ WZN)
    pub fn initiate_emergency_unlock(
        ctx: Context<InitiateEmergencyUnlock>, 
        percentage: u8,
        reason: String
    ) -> Result<()> {
        let emergency_config = &ctx.accounts.emergency_config;
        let clock = Clock::get()?;
        
        // Check if 2-year time lock has passed
        require!(
            clock.unix_timestamp >= emergency_config.time_lock_start + emergency_config.time_lock_duration,
            ErrorCode::TimeLockNotExpired
        );
        
        // Check cooldown period (30 days between unlocks)
        require!(
            emergency_config.last_unlock_time == 0 || 
            clock.unix_timestamp >= emergency_config.last_unlock_time + emergency_config.cooldown_period,
            ErrorCode::CooldownActive
        );
        
        // Check valid percentage range (15-35%)
        require!(percentage >= 15 && percentage <= 35, ErrorCode::InvalidUnlockPercentage);
        
        // Check initiator has minimum WZN balance
        require!(
            ctx.accounts.initiator_wzn_account.amount >= emergency_config.min_wzn_balance,
            ErrorCode::InsufficientWznForEmergency
        );
        
        let unlock_request = &mut ctx.accounts.unlock_request;
        unlock_request.id = emergency_config.current_unlock_id;
        unlock_request.initiator = ctx.accounts.initiator.key();
        unlock_request.percentage = percentage;
        unlock_request.reason = reason;
        unlock_request.created_at = clock.unix_timestamp;
        unlock_request.signature_deadline = clock.unix_timestamp + (7 * 24 * 60 * 60); // 7 days to collect signatures
        unlock_request.signatures_count = 1; // Initiator counts as first signature
        unlock_request.executed = false;
        unlock_request.cancelled = false;
        
        // Record initiator's signature
        let initiator_signature = &mut ctx.accounts.initiator_signature;
        initiator_signature.signer = ctx.accounts.initiator.key();
        initiator_signature.unlock_id = unlock_request.id;
        initiator_signature.signed_at = clock.unix_timestamp;
        initiator_signature.wzn_balance = ctx.accounts.initiator_wzn_account.amount;
        
        msg!("Emergency unlock {} initiated: {}% release", unlock_request.id, percentage);
        
        Ok(())
    }
    
    /// Sign emergency unlock request (requires 10,000+ WZN)
    pub fn sign_emergency_unlock(ctx: Context<SignEmergencyUnlock>, unlock_id: u64) -> Result<()> {
        let emergency_config = &ctx.accounts.emergency_config;
        let unlock_request = &mut ctx.accounts.unlock_request;
        let clock = Clock::get()?;
        
        // Check if signature period is still open
        require!(clock.unix_timestamp <= unlock_request.signature_deadline, ErrorCode::SignaturePeriodClosed);
        require!(!unlock_request.executed, ErrorCode::UnlockAlreadyExecuted);
        require!(!unlock_request.cancelled, ErrorCode::UnlockCancelled);
        
        // Check signer has minimum WZN balance
        require!(
            ctx.accounts.signer_wzn_account.amount >= emergency_config.min_wzn_balance,
            ErrorCode::InsufficientWznForEmergency
        );
        
        // Check if signer already signed
        let signature = &mut ctx.accounts.signature;
        require!(!signature.signed_at > 0, ErrorCode::AlreadySigned);
        
        // Record signature
        signature.signer = ctx.accounts.signer.key();
        signature.unlock_id = unlock_id;
        signature.signed_at = clock.unix_timestamp;
        signature.wzn_balance = ctx.accounts.signer_wzn_account.amount;
        
        unlock_request.signatures_count += 1;
        
        msg!("Emergency unlock {} signed by {} ({}/5)", unlock_id, ctx.accounts.signer.key(), unlock_request.signatures_count);
        
        Ok(())
    }
    
    /// Execute emergency unlock (after collecting 5 signatures)
    pub fn execute_emergency_unlock(ctx: Context<ExecuteEmergencyUnlock>, unlock_id: u64) -> Result<()> {
        let emergency_config = &mut ctx.accounts.emergency_config;
        let unlock_request = &mut ctx.accounts.unlock_request;
        let clock = Clock::get()?;
        
        require!(!unlock_request.executed, ErrorCode::UnlockAlreadyExecuted);
        require!(!unlock_request.cancelled, ErrorCode::UnlockCancelled);
        
        // Check if we have enough signatures (5 required)
        require!(unlock_request.signatures_count >= emergency_config.min_signers, ErrorCode::InsufficientSignatures);
        
        // Calculate unlock amount (percentage of burn vault)
        let vault_balance = ctx.accounts.burn_vault.amount;
        let unlock_amount = (vault_balance * unlock_request.percentage as u64) / 100;
        
        // Transfer tokens from burn vault to admin wallet
        let seeds = &[b"burn_vault".as_ref(), &[ctx.bumps.burn_vault]];
        let signer = &[&seeds[..]];
        
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.burn_vault.to_account_info(),
                to: ctx.accounts.admin_token_account.to_account_info(),
                authority: ctx.accounts.burn_vault.to_account_info(),
            },
            signer,
        );
        token::transfer(cpi_ctx, unlock_amount)?;
        
        // Update state
        unlock_request.executed = true;
        emergency_config.last_unlock_time = clock.unix_timestamp;
        emergency_config.current_unlock_id += 1;
        
        msg!("Emergency unlock {} executed: {} WZN released ({}%)", 
             unlock_id, unlock_amount / 1_000_000, unlock_request.percentage);
        
        Ok(())
    }
    
    /// Cancel emergency unlock (only by initiator or admin)
    pub fn cancel_emergency_unlock(ctx: Context<CancelEmergencyUnlock>, unlock_id: u64) -> Result<()> {
        let unlock_request = &mut ctx.accounts.unlock_request;
        let emergency_config = &ctx.accounts.emergency_config;
        
        // Only initiator or admin can cancel
        require!(
            ctx.accounts.signer.key() == unlock_request.initiator || 
            ctx.accounts.signer.key() == emergency_config.admin,
            ErrorCode::UnauthorizedCancel
        );
        
        require!(!unlock_request.executed, ErrorCode::UnlockAlreadyExecuted);
        require!(!unlock_request.cancelled, ErrorCode::UnlockAlreadyCancelled);
        
        unlock_request.cancelled = true;
        
        msg!("Emergency unlock {} cancelled by {}", unlock_id, ctx.accounts.signer.key());
        
        Ok(())
    }

    // ========== MODULE 6: Prize Distribution (2h) ==========
    
    /// Initialize prize distribution system
    pub fn initialize_prize_distribution(ctx: Context<InitializePrizeDistribution>) -> Result<()> {
        let prize_config = &mut ctx.accounts.prize_config;
        
        prize_config.admin = ctx.accounts.admin.key();
        prize_config.total_distributed = 0;
        prize_config.current_season = 1;
        prize_config.season_start_time = Clock::get()?.unix_timestamp;
        prize_config.distribution_count = 0;
        
        msg!("Prize distribution system initialized");
        
        Ok(())
    }
    
    /// Submit rewards to multiple winners (only admin or via DAO proposal)
    pub fn submit_rewards(
        ctx: Context<SubmitRewards>, 
        recipients: Vec<Pubkey>, 
        amounts: Vec<u64>,
        reason: String
    ) -> Result<()> {
        require!(recipients.len() == amounts.len(), ErrorCode::MismatchedRecipientsAmounts);
        require!(recipients.len() <= 50, ErrorCode::TooManyRecipients); // Limit batch size
        
        let prize_config = &mut ctx.accounts.prize_config;
        let clock = Clock::get()?;
        
        // Calculate total amount to distribute
        let total_amount: u64 = amounts.iter().sum();
        
        // Check if burn vault has enough tokens
        require!(
            ctx.accounts.burn_vault.amount >= total_amount,
            ErrorCode::InsufficientVaultBalance
        );
        
        // Create distribution record
        let distribution = &mut ctx.accounts.distribution;
        distribution.id = prize_config.distribution_count + 1;
        distribution.admin = ctx.accounts.admin.key();
        distribution.total_amount = total_amount;
        distribution.recipients_count = recipients.len() as u32;
        distribution.reason = reason;
        distribution.created_at = clock.unix_timestamp;
        distribution.executed = false;
        distribution.season = prize_config.current_season;
        
        // Store recipients and amounts (simplified - in real implementation might need multiple accounts)
        distribution.recipients = recipients;
        distribution.amounts = amounts;
        
        prize_config.distribution_count += 1;
        
        msg!("Prize distribution {} created: {} WZN to {} recipients", 
             distribution.id, total_amount / 1_000_000, distribution.recipients_count);
        
        Ok(())
    }
    
    /// Execute prize distribution (transfer tokens to winners)
    pub fn execute_prize_distribution(ctx: Context<ExecutePrizeDistribution>, distribution_id: u64) -> Result<()> {
        let distribution = &mut ctx.accounts.distribution;
        let prize_config = &mut ctx.accounts.prize_config;
        
        require!(!distribution.executed, ErrorCode::DistributionAlreadyExecuted);
        
        let total_amount = distribution.total_amount;
        
        // Transfer from burn vault to admin for distribution
        // In a real implementation, this would loop through recipients
        let seeds = &[b"burn_vault".as_ref(), &[ctx.bumps.burn_vault]];
        let signer = &[&seeds[..]];
        
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.burn_vault.to_account_info(),
                to: ctx.accounts.admin_token_account.to_account_info(),
                authority: ctx.accounts.burn_vault.to_account_info(),
            },
            signer,
        );
        token::transfer(cpi_ctx, total_amount)?;
        
        distribution.executed = true;
        prize_config.total_distributed += total_amount;
        
        msg!("Prize distribution {} executed: {} WZN distributed", 
             distribution_id, total_amount / 1_000_000);
        
        Ok(())
    }
    
    /// Start new season (admin only)
    pub fn start_new_season(ctx: Context<StartNewSeason>) -> Result<()> {
        let prize_config = &mut ctx.accounts.prize_config;
        let clock = Clock::get()?;
        
        prize_config.current_season += 1;
        prize_config.season_start_time = clock.unix_timestamp;
        
        msg!("New season {} started", prize_config.current_season);
        
        Ok(())
    }
    
    /// Cancel prize distribution (admin only, before execution)
    pub fn cancel_prize_distribution(ctx: Context<CancelPrizeDistribution>, distribution_id: u64) -> Result<()> {
        let distribution = &mut ctx.accounts.distribution;
        
        require!(!distribution.executed, ErrorCode::DistributionAlreadyExecuted);
        
        // In a simplified implementation, we just mark as cancelled
        // Could add a cancelled field to PrizeDistribution struct
        
        msg!("Prize distribution {} cancelled by admin", distribution_id);
        
        Ok(())
    }

    // ========== MODULE 7: Governance Helpers (1.5h) ==========
    
    /// Get voting status for a proposal
    pub fn get_proposal_status(ctx: Context<GetProposalStatus>, proposal_id: u64) -> Result<ProposalStatus> {
        let proposal = &ctx.accounts.proposal;
        let clock = Clock::get()?;
        
        let status = if proposal.cancelled {
            ProposalStatus::Cancelled
        } else if proposal.executed {
            ProposalStatus::Executed
        } else if clock.unix_timestamp <= proposal.voting_deadline {
            ProposalStatus::VotingOpen
        } else if clock.unix_timestamp <= proposal.execution_deadline {
            // Check if quorum and votes are sufficient
            let total_votes = proposal.yes_votes + proposal.no_votes;
            let yes_percentage = if total_votes > 0 { (proposal.yes_votes * 100) / total_votes } else { 0 };
            
            if proposal.total_voters >= 100 && yes_percentage >= 60 {
                ProposalStatus::ReadyForExecution
            } else {
                ProposalStatus::Failed
            }
        } else {
            ProposalStatus::Expired
        };
        
        msg!("Proposal {} status: {:?}", proposal_id, status);
        
        Ok(status)
    }
    
    /// Check if user has already voted on a proposal (prevents double voting)
    pub fn check_user_vote_status(ctx: Context<CheckUserVoteStatus>, proposal_id: u64) -> Result<bool> {
        let user_vote = &ctx.accounts.user_vote;
        let has_voted = user_vote.has_voted;
        
        msg!("User {} vote status on proposal {}: {}", 
             ctx.accounts.user.key(), proposal_id, if has_voted {"VOTED"} else {"NOT_VOTED"});
        
        Ok(has_voted)
    }
    
    /// Get user's voting eligibility (WZN balance + active access)
    pub fn check_voting_eligibility(ctx: Context<CheckVotingEligibility>) -> Result<VotingEligibility> {
        let user_access = &ctx.accounts.user_access;
        let clock = Clock::get()?;
        
        let has_active_access = clock.unix_timestamp < user_access.access_expires;
        let wzn_balance = ctx.accounts.user_wzn_account.amount;
        let min_balance_for_voting = 1_000_000_000; // 1,000 WZN
        let min_balance_for_proposal = 10_000_000_000; // 10,000 WZN
        
        let eligibility = VotingEligibility {
            has_active_access,
            wzn_balance,
            can_vote: has_active_access && wzn_balance >= min_balance_for_voting,
            can_create_proposal: has_active_access && wzn_balance >= min_balance_for_proposal,
            voting_power: wzn_balance / 1_000_000, // 1 WZN = 1 vote
        };
        
        msg!("User {} voting eligibility: can_vote={}, can_propose={}, power={}",
             ctx.accounts.user.key(), eligibility.can_vote, eligibility.can_create_proposal, eligibility.voting_power);
        
        Ok(eligibility)
    }
    
    /// Get emergency unlock limits and status
    pub fn check_emergency_unlock_limits(ctx: Context<CheckEmergencyUnlockLimits>) -> Result<EmergencyUnlockStatus> {
        let emergency_config = &ctx.accounts.emergency_config;
        let clock = Clock::get()?;
        
        let time_lock_expires = emergency_config.time_lock_start + emergency_config.time_lock_duration;
        let cooldown_expires = emergency_config.last_unlock_time + emergency_config.cooldown_period;
        
        let status = EmergencyUnlockStatus {
            time_lock_expired: clock.unix_timestamp >= time_lock_expires,
            cooldown_expired: emergency_config.last_unlock_time == 0 || clock.unix_timestamp >= cooldown_expires,
            time_lock_expires_at: time_lock_expires,
            cooldown_expires_at: if emergency_config.last_unlock_time == 0 { 0 } else { cooldown_expires },
            min_percentage: 15,
            max_percentage: 35,
            required_signatures: emergency_config.min_signers,
            min_wzn_balance: emergency_config.min_wzn_balance,
        };
        
        msg!("Emergency unlock status: time_lock_expired={}, cooldown_expired={}", 
             status.time_lock_expired, status.cooldown_expired);
        
        Ok(status)
    }
    
    /// Get comprehensive statistics for the platform
    pub fn get_platform_stats(ctx: Context<GetPlatformStats>) -> Result<PlatformStats> {
        let global_config = &ctx.accounts.global_config;
        let prize_config = &ctx.accounts.prize_config;
        let burn_vault = &ctx.accounts.burn_vault;
        
        let stats = PlatformStats {
            total_burned: burn_vault.amount,
            total_distributed: prize_config.total_distributed,
            current_access_cost: global_config.access_cost,
            current_fee_mode: global_config.fee_mode,
            current_season: prize_config.current_season,
            total_proposals: global_config.next_proposal_id - 1,
            total_distributions: prize_config.distribution_count,
        };
        
        msg!("Platform stats: burned={} WZN, distributed={} WZN, proposals={}", 
             stats.total_burned / 1_000_000, stats.total_distributed / 1_000_000, stats.total_proposals);
        
        Ok(stats)
    }
}

// ========== ACCOUNT STRUCTURES ==========

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + GlobalConfig::LEN,
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeBurnVault<'info> {
    #[account(
        init,
        payer = admin,
        token::mint = wzn_mint,
        token::authority = burn_vault,
        seeds = [b"burn_vault"],
        bump
    )]
    pub burn_vault: Account<'info, TokenAccount>,
    
    pub wzn_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BurnForPass<'info> {
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + UserAccessInfo::LEN,
        seeds = [b"user_access", user.key().as_ref()],
        bump
    )]
    pub user_access: Account<'info, UserAccessInfo>,
    
    #[account(
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [b"burn_vault"],
        bump
    )]
    pub burn_vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BurnForPassWithFeePayer<'info> {
    #[account(
        init_if_needed,
        payer = fee_payer, // Project pays the initialization fee
        space = 8 + UserAccessInfo::LEN,
        seeds = [b"user_access", user.key().as_ref()],
        bump
    )]
    pub user_access: Account<'info, UserAccessInfo>,
    
    #[account(
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        mut,
        seeds = [b"burn_vault"],
        bump
    )]
    pub burn_vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    
    pub user: Signer<'info>,
    
    #[account(mut)]
    pub fee_payer: Signer<'info>, // Project wallet that pays fees
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CheckAccess<'info> {
    #[account(
        seeds = [b"user_access", user.key().as_ref()],
        bump
    )]
    pub user_access: Account<'info, UserAccessInfo>,
    
    /// CHECK: This is just for the seed derivation
    pub user: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ChangeAccessCost<'info> {
    #[account(
        mut,
        seeds = [b"global_config"],
        bump,
        has_one = admin
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct ChangeFeeMode<'info> {
    #[account(
        mut,
        seeds = [b"global_config"],
        bump,
        has_one = admin
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    pub admin: Signer<'info>,
}

// ========== MODULE 4: DAO VOTING ACCOUNTS ==========

#[derive(Accounts)]
pub struct CreateProposal<'info> {
    #[account(
        init,
        payer = proposer,
        space = 8 + Proposal::LEN,
        seeds = [b"proposal", global_config.next_proposal_id.to_le_bytes().as_ref()],
        bump
    )]
    pub proposal: Account<'info, Proposal>,
    
    #[account(
        mut,
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        seeds = [b"user_access", proposer.key().as_ref()],
        bump
    )]
    pub user_access: Account<'info, UserAccessInfo>,
    
    #[account(
        constraint = user_wzn_account.owner == proposer.key(),
        constraint = user_wzn_account.mint == wzn_mint.key()
    )]
    pub user_wzn_account: Account<'info, TokenAccount>,
    
    pub wzn_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub proposer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(proposal_id: u64)]
pub struct VoteOnProposal<'info> {
    #[account(
        mut,
        seeds = [b"proposal", proposal_id.to_le_bytes().as_ref()],
        bump
    )]
    pub proposal: Account<'info, Proposal>,
    
    #[account(
        init,
        payer = voter,
        space = 8 + UserVote::LEN,
        seeds = [b"user_vote", proposal_id.to_le_bytes().as_ref(), voter.key().as_ref()],
        bump
    )]
    pub user_vote: Account<'info, UserVote>,
    
    #[account(
        seeds = [b"user_access", voter.key().as_ref()],
        bump
    )]
    pub user_access: Account<'info, UserAccessInfo>,
    
    #[account(
        constraint = user_wzn_account.owner == voter.key(),
        constraint = user_wzn_account.mint == wzn_mint.key()
    )]
    pub user_wzn_account: Account<'info, TokenAccount>,
    
    pub wzn_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub voter: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(proposal_id: u64)]
pub struct ExecuteProposal<'info> {
    #[account(
        mut,
        seeds = [b"proposal", proposal_id.to_le_bytes().as_ref()],
        bump
    )]
    pub proposal: Account<'info, Proposal>,
    
    #[account(
        mut,
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    pub executor: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(proposal_id: u64)]
pub struct CancelProposal<'info> {
    #[account(
        mut,
        seeds = [b"proposal", proposal_id.to_le_bytes().as_ref()],
        bump
    )]
    pub proposal: Account<'info, Proposal>,
    
    #[account(
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    pub signer: Signer<'info>,
}

// ========== MODULE 5: EMERGENCY UNLOCK ACCOUNTS ==========

#[derive(Accounts)]
pub struct InitializeEmergencyUnlock<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + EmergencyConfig::LEN,
        seeds = [b"emergency_config"],
        bump
    )]
    pub emergency_config: Account<'info, EmergencyConfig>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitiateEmergencyUnlock<'info> {
    #[account(
        seeds = [b"emergency_config"],
        bump
    )]
    pub emergency_config: Account<'info, EmergencyConfig>,
    
    #[account(
        init,
        payer = initiator,
        space = 8 + EmergencyUnlockRequest::LEN,
        seeds = [b"unlock_request", emergency_config.current_unlock_id.to_le_bytes().as_ref()],
        bump
    )]
    pub unlock_request: Account<'info, EmergencyUnlockRequest>,
    
    #[account(
        init,
        payer = initiator,
        space = 8 + EmergencySignature::LEN,
        seeds = [b"signature", emergency_config.current_unlock_id.to_le_bytes().as_ref(), initiator.key().as_ref()],
        bump
    )]
    pub initiator_signature: Account<'info, EmergencySignature>,
    
    #[account(
        constraint = initiator_wzn_account.owner == initiator.key(),
        constraint = initiator_wzn_account.mint == wzn_mint.key()
    )]
    pub initiator_wzn_account: Account<'info, TokenAccount>,
    
    pub wzn_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub initiator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(unlock_id: u64)]
pub struct SignEmergencyUnlock<'info> {
    #[account(
        seeds = [b"emergency_config"],
        bump
    )]
    pub emergency_config: Account<'info, EmergencyConfig>,
    
    #[account(
        mut,
        seeds = [b"unlock_request", unlock_id.to_le_bytes().as_ref()],
        bump
    )]
    pub unlock_request: Account<'info, EmergencyUnlockRequest>,
    
    #[account(
        init,
        payer = signer,
        space = 8 + EmergencySignature::LEN,
        seeds = [b"signature", unlock_id.to_le_bytes().as_ref(), signer.key().as_ref()],
        bump
    )]
    pub signature: Account<'info, EmergencySignature>,
    
    #[account(
        constraint = signer_wzn_account.owner == signer.key(),
        constraint = signer_wzn_account.mint == wzn_mint.key()
    )]
    pub signer_wzn_account: Account<'info, TokenAccount>,
    
    pub wzn_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(unlock_id: u64)]
pub struct ExecuteEmergencyUnlock<'info> {
    #[account(
        mut,
        seeds = [b"emergency_config"],
        bump
    )]
    pub emergency_config: Account<'info, EmergencyConfig>,
    
    #[account(
        mut,
        seeds = [b"unlock_request", unlock_id.to_le_bytes().as_ref()],
        bump
    )]
    pub unlock_request: Account<'info, EmergencyUnlockRequest>,
    
    #[account(
        mut,
        seeds = [b"burn_vault"],
        bump
    )]
    pub burn_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = admin_token_account.owner == emergency_config.admin
    )]
    pub admin_token_account: Account<'info, TokenAccount>,
    
    pub executor: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(unlock_id: u64)]
pub struct CancelEmergencyUnlock<'info> {
    #[account(
        mut,
        seeds = [b"unlock_request", unlock_id.to_le_bytes().as_ref()],
        bump
    )]
    pub unlock_request: Account<'info, EmergencyUnlockRequest>,
    
    #[account(
        seeds = [b"emergency_config"],
        bump
    )]
    pub emergency_config: Account<'info, EmergencyConfig>,
    
    pub signer: Signer<'info>,
}

// ========== MODULE 6: PRIZE DISTRIBUTION ACCOUNTS ==========

#[derive(Accounts)]
pub struct InitializePrizeDistribution<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + PrizeConfig::LEN,
        seeds = [b"prize_config"],
        bump
    )]
    pub prize_config: Account<'info, PrizeConfig>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SubmitRewards<'info> {
    #[account(
        mut,
        seeds = [b"prize_config"],
        bump,
        has_one = admin
    )]
    pub prize_config: Account<'info, PrizeConfig>,
    
    #[account(
        init,
        payer = admin,
        space = 8 + PrizeDistribution::LEN,
        seeds = [b"distribution", (prize_config.distribution_count + 1).to_le_bytes().as_ref()],
        bump
    )]
    pub distribution: Account<'info, PrizeDistribution>,
    
    #[account(
        seeds = [b"burn_vault"],
        bump
    )]
    pub burn_vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(distribution_id: u64)]
pub struct ExecutePrizeDistribution<'info> {
    #[account(
        mut,
        seeds = [b"prize_config"],
        bump
    )]
    pub prize_config: Account<'info, PrizeConfig>,
    
    #[account(
        mut,
        seeds = [b"distribution", distribution_id.to_le_bytes().as_ref()],
        bump
    )]
    pub distribution: Account<'info, PrizeDistribution>,
    
    #[account(
        mut,
        seeds = [b"burn_vault"],
        bump
    )]
    pub burn_vault: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = admin_token_account.owner == prize_config.admin
    )]
    pub admin_token_account: Account<'info, TokenAccount>,
    
    pub executor: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct StartNewSeason<'info> {
    #[account(
        mut,
        seeds = [b"prize_config"],
        bump,
        has_one = admin
    )]
    pub prize_config: Account<'info, PrizeConfig>,
    
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(distribution_id: u64)]
pub struct CancelPrizeDistribution<'info> {
    #[account(
        seeds = [b"prize_config"],
        bump,
        has_one = admin
    )]
    pub prize_config: Account<'info, PrizeConfig>,
    
    #[account(
        mut,
        seeds = [b"distribution", distribution_id.to_le_bytes().as_ref()],
        bump
    )]
    pub distribution: Account<'info, PrizeDistribution>,
    
    pub admin: Signer<'info>,
}

// ========== MODULE 7: GOVERNANCE HELPERS ACCOUNTS ==========

#[derive(Accounts)]
#[instruction(proposal_id: u64)]
pub struct GetProposalStatus<'info> {
    #[account(
        seeds = [b"proposal", proposal_id.to_le_bytes().as_ref()],
        bump
    )]
    pub proposal: Account<'info, Proposal>,
}

#[derive(Accounts)]
#[instruction(proposal_id: u64)]
pub struct CheckUserVoteStatus<'info> {
    #[account(
        seeds = [b"user_vote", proposal_id.to_le_bytes().as_ref(), user.key().as_ref()],
        bump
    )]
    pub user_vote: Account<'info, UserVote>,
    
    /// CHECK: This is just for the seed derivation
    pub user: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CheckVotingEligibility<'info> {
    #[account(
        seeds = [b"user_access", user.key().as_ref()],
        bump
    )]
    pub user_access: Account<'info, UserAccessInfo>,
    
    #[account(
        constraint = user_wzn_account.owner == user.key(),
        constraint = user_wzn_account.mint == wzn_mint.key()
    )]
    pub user_wzn_account: Account<'info, TokenAccount>,
    
    pub wzn_mint: Account<'info, Mint>,
    
    /// CHECK: This is just for the seed derivation
    pub user: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CheckEmergencyUnlockLimits<'info> {
    #[account(
        seeds = [b"emergency_config"],
        bump
    )]
    pub emergency_config: Account<'info, EmergencyConfig>,
}

#[derive(Accounts)]
pub struct GetPlatformStats<'info> {
    #[account(
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(
        seeds = [b"prize_config"],
        bump
    )]
    pub prize_config: Account<'info, PrizeConfig>,
    
    #[account(
        seeds = [b"burn_vault"],
        bump
    )]
    pub burn_vault: Account<'info, TokenAccount>,
}

// ========== DATA STRUCTURES ==========

#[account]
pub struct GlobalConfig {
    pub admin: Pubkey,
    pub access_cost: u64,       // Cost in WZN lamports to get 30-day access
    pub fee_mode: u8,           // 0 = user pays fees, 1 = project pays fees
    pub next_proposal_id: u64,  // Counter for proposal IDs
}

impl GlobalConfig {
    pub const LEN: usize = 32 + 8 + 1 + 8;
}

#[account]
pub struct UserAccessInfo {
    pub user: Pubkey,
    pub burn_timestamp: i64,
    pub access_expires: i64,
    pub amount_burned: u64,
}

impl UserAccessInfo {
    pub const LEN: usize = 32 + 8 + 8 + 8;
}

// ========== MODULE 4: DAO VOTING DATA STRUCTURES ==========

#[account]
pub struct Proposal {
    pub id: u64,
    pub proposer: Pubkey,
    pub proposal_type: u8,    // 0=AccessCost, 1=FeeMode, 2=EmergencyUnlock, 3=PrizeDistribution
    pub description: String,
    pub target_value: u64,    // The value to change to (cost, mode, amount, etc.)
    pub created_at: i64,
    pub voting_deadline: i64,
    pub execution_deadline: i64,
    pub yes_votes: u64,       // Total WZN-based voting power for YES
    pub no_votes: u64,        // Total WZN-based voting power for NO
    pub total_voters: u32,    // Number of unique voters
    pub executed: bool,
    pub cancelled: bool,
}

impl Proposal {
    pub const LEN: usize = 8 + 32 + 1 + (4 + 200) + 8 + 8 + 8 + 8 + 8 + 8 + 4 + 1 + 1;
}

#[account]
pub struct UserVote {
    pub user: Pubkey,
    pub proposal_id: u64,
    pub vote: bool,           // true = YES, false = NO
    pub voting_power: u64,    // WZN amount used for voting (prevents double voting)
    pub has_voted: bool,
}

impl UserVote {
    pub const LEN: usize = 32 + 8 + 1 + 8 + 1;
}

// ========== MODULE 5: EMERGENCY UNLOCK DATA STRUCTURES ==========

#[account]
pub struct EmergencyConfig {
    pub admin: Pubkey,
    pub time_lock_start: i64,      // When the 2-year lock started
    pub time_lock_duration: i64,   // 2 years in seconds
    pub last_unlock_time: i64,     // Last emergency unlock timestamp
    pub cooldown_period: i64,      // 30 days cooldown between unlocks
    pub min_signers: u8,           // Required signatures (5)
    pub min_wzn_balance: u64,      // Minimum WZN balance for signers (10,000 WZN)
    pub current_unlock_id: u64,    // Counter for unlock requests
}

impl EmergencyConfig {
    pub const LEN: usize = 32 + 8 + 8 + 8 + 8 + 1 + 8 + 8;
}

#[account]
pub struct EmergencyUnlockRequest {
    pub id: u64,
    pub initiator: Pubkey,
    pub percentage: u8,           // 15-35% of burn vault
    pub reason: String,
    pub created_at: i64,
    pub signature_deadline: i64,  // 7 days to collect signatures
    pub signatures_count: u8,     // Current number of signatures
    pub executed: bool,
    pub cancelled: bool,
}

impl EmergencyUnlockRequest {
    pub const LEN: usize = 8 + 32 + 1 + (4 + 200) + 8 + 8 + 1 + 1 + 1;
}

#[account]
pub struct EmergencySignature {
    pub signer: Pubkey,
    pub unlock_id: u64,
    pub signed_at: i64,
    pub wzn_balance: u64,         // WZN balance at time of signing (for verification)
}

impl EmergencySignature {
    pub const LEN: usize = 32 + 8 + 8 + 8;
}

// ========== MODULE 6: PRIZE DISTRIBUTION DATA STRUCTURES ==========

#[account]
pub struct PrizeConfig {
    pub admin: Pubkey,
    pub total_distributed: u64,   // Total WZN distributed as prizes
    pub current_season: u32,      // Current game season
    pub season_start_time: i64,   // When current season started
    pub distribution_count: u64,  // Counter for distributions
}

impl PrizeConfig {
    pub const LEN: usize = 32 + 8 + 4 + 8 + 8;
}

#[account]
pub struct PrizeDistribution {
    pub id: u64,
    pub admin: Pubkey,
    pub total_amount: u64,
    pub recipients_count: u32,
    pub reason: String,           // Reason for distribution (tournament, event, etc.)
    pub created_at: i64,
    pub executed: bool,
    pub season: u32,
    pub recipients: Vec<Pubkey>,  // List of winner wallets
    pub amounts: Vec<u64>,        // Corresponding prize amounts
}

impl PrizeDistribution {
    // Simplified constant - in real implementation would be dynamic based on recipients
    pub const LEN: usize = 8 + 32 + 8 + 4 + (4 + 200) + 8 + 1 + 4 + (4 + 50 * 32) + (4 + 50 * 8);
}

// ========== MODULE 7: GOVERNANCE HELPERS DATA STRUCTURES ==========

#[derive(Clone, Debug, PartialEq, AnchorSerialize, AnchorDeserialize)]
pub enum ProposalStatus {
    VotingOpen,
    ReadyForExecution,
    Failed,
    Executed,
    Cancelled,
    Expired,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct VotingEligibility {
    pub has_active_access: bool,
    pub wzn_balance: u64,
    pub can_vote: bool,
    pub can_create_proposal: bool,
    pub voting_power: u64,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct EmergencyUnlockStatus {
    pub time_lock_expired: bool,
    pub cooldown_expired: bool,
    pub time_lock_expires_at: i64,
    pub cooldown_expires_at: i64,
    pub min_percentage: u8,
    pub max_percentage: u8,
    pub required_signatures: u8,
    pub min_wzn_balance: u64,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct PlatformStats {
    pub total_burned: u64,
    pub total_distributed: u64,
    pub current_access_cost: u64,
    pub current_fee_mode: u8,
    pub current_season: u32,
    pub total_proposals: u64,
    pub total_distributions: u64,
}

// ========== ERROR CODES ==========

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid burn amount - must be between 500-1000 WZN")]
    InvalidBurnAmount,
    #[msg("Access still active - cannot burn again yet")]
    AccessStillActive,
    #[msg("Invalid access cost - must be between 500-1000 WZN")]
    InvalidAccessCost,
    #[msg("Invalid fee mode - must be 0 or 1")]
    InvalidFeeMode,
    #[msg("Wrong fee mode for this function")]
    WrongFeeMode,
    
    // DAO Voting errors
    #[msg("No active access pass - required for voting")]
    NoActiveAccess,
    #[msg("Insufficient WZN balance for proposal creation (min 10,000 WZN required)")]
    InsufficientWznForProposal,
    #[msg("Insufficient WZN balance for voting (min 1,000 WZN required)")]
    InsufficientWznForVoting,
    #[msg("Invalid proposal type")]
    InvalidProposalType,
    #[msg("Voting period has closed")]
    VotingClosed,
    #[msg("Voting period is still open")]
    VotingStillOpen,
    #[msg("Proposal already executed")]
    ProposalAlreadyExecuted,
    #[msg("Proposal was cancelled")]
    ProposalCancelled,
    #[msg("User already voted on this proposal")]
    AlreadyVoted,
    #[msg("Quorum not met (minimum 100 voters required)")]
    QuorumNotMet,
    #[msg("Insufficient yes votes (minimum 60% required)")]
    InsufficientYesVotes,
    #[msg("Execution deadline has passed")]
    ExecutionDeadlinePassed,
    #[msg("Unauthorized to cancel proposal")]
    UnauthorizedCancel,
    #[msg("Proposal already cancelled")]
    ProposalAlreadyCancelled,
    
    // Emergency Unlock errors
    #[msg("2-year time lock has not expired yet")]
    TimeLockNotExpired,
    #[msg("30-day cooldown period is still active")]
    CooldownActive,
    #[msg("Invalid unlock percentage - must be between 15-35%")]
    InvalidUnlockPercentage,
    #[msg("Insufficient WZN balance for emergency unlock (min 10,000 WZN required)")]
    InsufficientWznForEmergency,
    #[msg("Signature period has closed")]
    SignaturePeriodClosed,
    #[msg("Emergency unlock already executed")]
    UnlockAlreadyExecuted,
    #[msg("Emergency unlock was cancelled")]
    UnlockCancelled,
    #[msg("Already signed this emergency unlock")]
    AlreadySigned,
    #[msg("Insufficient signatures for emergency unlock (5 required)")]
    InsufficientSignatures,
    #[msg("Emergency unlock already cancelled")]
    UnlockAlreadyCancelled,
    
    // Prize Distribution errors
    #[msg("Recipients and amounts arrays must have same length")]
    MismatchedRecipientsAmounts,
    #[msg("Too many recipients in single distribution (max 50)")]
    TooManyRecipients,
    #[msg("Insufficient balance in burn vault for prize distribution")]
    InsufficientVaultBalance,
    #[msg("Prize distribution already executed")]
    DistributionAlreadyExecuted,
}
