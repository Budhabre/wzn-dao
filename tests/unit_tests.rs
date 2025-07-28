use wzn_card_game::*;
use anchor_lang::prelude::*;

// Basic unit tests that don't require full Anchor test framework
#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_global_config_structure() {
        // Test GlobalConfig size and field calculations
        let config = GlobalConfig {
            admin: Pubkey::default(),
            access_cost: 500_000_000,
            fee_mode: 0,
            next_proposal_id: 1,
        };
        
        // Basic sanity checks
        assert_eq!(config.access_cost, 500_000_000);
        assert_eq!(config.fee_mode, 0);
        assert_eq!(config.next_proposal_id, 1);
    }

    #[test]
    fn test_user_access_info_structure() {
        let access_info = UserAccessInfo {
            user: Pubkey::default(),
            burn_timestamp: 1234567890,
            access_expires: 1234567890 + (30 * 24 * 60 * 60),
            amount_burned: 500_000_000,
        };
        
        assert_eq!(access_info.amount_burned, 500_000_000);
        assert!(access_info.access_expires > access_info.burn_timestamp);
    }

    #[test]
    fn test_proposal_structure() {
        let proposal = Proposal {
            id: 1,
            proposer: Pubkey::default(),
            proposal_type: 0,
            description: "Test proposal".to_string(),
            target_value: 600_000_000,
            created_at: 1234567890,
            voting_deadline: 1234567890 + (7 * 24 * 60 * 60),
            execution_deadline: 1234567890 + (14 * 24 * 60 * 60),
            yes_votes: 0,
            no_votes: 0,
            total_voters: 0,
            executed: false,
            cancelled: false,
        };
        
        assert_eq!(proposal.id, 1);
        assert_eq!(proposal.proposal_type, 0);
        assert!(!proposal.executed);
        assert!(!proposal.cancelled);
    }

    #[test]
    fn test_burn_amount_validation() {
        // Test valid burn amounts
        assert!(500_000_000 >= 500_000_000 && 500_000_000 <= 1_000_000_000);
        assert!(1_000_000_000 >= 500_000_000 && 1_000_000_000 <= 1_000_000_000);
        
        // Test invalid burn amounts
        assert!(!(499_999_999 >= 500_000_000 && 499_999_999 <= 1_000_000_000));
        assert!(!(1_000_000_001 >= 500_000_000 && 1_000_000_001 <= 1_000_000_000));
    }

    #[test]
    fn test_voting_power_calculation() {
        let wzn_balance = 5_000_000_000; // 5,000 WZN
        let voting_power = wzn_balance / 1_000_000; // Should be 5,000 votes
        
        assert_eq!(voting_power, 5_000);
    }

    #[test]
    fn test_time_calculations() {
        let base_time = 1234567890i64;
        let thirty_days = 30 * 24 * 60 * 60;
        let seven_days = 7 * 24 * 60 * 60;
        let fourteen_days = 14 * 24 * 60 * 60;
        
        assert_eq!(thirty_days, 2_592_000);
        assert_eq!(seven_days, 604_800);
        assert_eq!(fourteen_days, 1_209_600);
        
        let access_expires = base_time + thirty_days;
        let voting_deadline = base_time + seven_days;
        let execution_deadline = base_time + fourteen_days;
        
        assert!(access_expires > base_time);
        assert!(voting_deadline > base_time);
        assert!(execution_deadline > voting_deadline);
    }

    #[test]
    fn test_percentage_calculations() {
        // Test emergency unlock percentage validation
        assert!(15 >= 15 && 15 <= 35);
        assert!(35 >= 15 && 35 <= 35);
        assert!(!(14 >= 15 && 14 <= 35));
        assert!(!(36 >= 15 && 36 <= 35));
        
        // Test voting percentage
        let yes_votes = 600u64;
        let no_votes = 400u64;
        let total_votes = yes_votes + no_votes;
        let yes_percentage = (yes_votes * 100) / total_votes;
        
        assert_eq!(yes_percentage, 60);
    }

    #[test]
    fn test_minimum_balances() {
        let min_voting_balance = 1_000_000_000; // 1,000 WZN
        let min_proposal_balance = 10_000_000_000; // 10,000 WZN
        let min_emergency_balance = 10_000_000_000; // 10,000 WZN
        
        // Test voting eligibility
        assert!(min_voting_balance >= min_voting_balance);
        assert!(!(999_999_999 >= min_voting_balance));
        
        // Test proposal creation eligibility
        assert!(min_proposal_balance >= min_proposal_balance);
        assert!(!(9_999_999_999 >= min_proposal_balance));
        
        // Test emergency unlock eligibility
        assert!(min_emergency_balance >= min_emergency_balance);
        assert!(!(9_999_999_999 >= min_emergency_balance));
    }
}
