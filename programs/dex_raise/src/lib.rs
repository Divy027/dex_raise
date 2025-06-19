use anchor_lang::prelude::*;

declare_id!("6h6tptXmE1g9F6pez3yaZYxdvdkBQRFnw3VH1Fcs3uze"); // Replace with your new program ID

#[program]
pub mod dexscreener_escrow{
    use super::*;

    // Initializes the global configuration. Only holds admin and fee wallet now.
    pub fn initialize(ctx: Context<Initialize>, fee_wallet: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.fee_wallet = fee_wallet;
        config.bump = ctx.bumps.config;
        Ok(())
    }

    // Creates a new fundraising campaign.
    pub fn create_campaign(
        ctx: Context<CreateCampaign>,
        campaign_type: CampaignType,
        goal_amount: u64,
    ) -> Result<()> {
        // Uniqueness check is handled by the `init` constraint on the campaign PDA.
        // If an active campaign with the same token mint and type exists, this instruction will fail.
        let campaign = &mut ctx.accounts.campaign;

        campaign.admin = ctx.accounts.config.admin;
        campaign.token_to_update_mint = ctx.accounts.token_to_update_mint.key();
        campaign.campaign_type = campaign_type;
        campaign.amount_raised = 0;
        campaign.goal_amount = goal_amount;
        campaign.is_active = false;
        campaign.is_funded = false;
        campaign.is_withdrawn = false;
        campaign.created_at = Clock::get()?.unix_timestamp; // Set creation time
        campaign.bump = ctx.bumps.campaign;

        Ok(())
    }

    pub fn set_funded_status(ctx: Context<UpdateCampaignStatus>) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
        let campaign_balance = campaign.to_account_info().lamports();

        require!(campaign_balance >= campaign.goal_amount, EscrowError::CampaignNotFunded);
        
        campaign.is_funded = true;
        campaign.is_active = false; // A funded campaign is no longer active for donations
        campaign.amount_raised = campaign_balance; // Sync the amount
        Ok(())
    }

    // `process_refund` sends SOL FROM the campaign PDA TO a user. Admin-only.
    pub fn process_refund(ctx: Context<ProcessRefund>, amount_to_refund: u64) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
        
        // Check if the campaign is expired and not funded. This is a common refund scenario.
        let now = Clock::get()?.unix_timestamp;
        let is_expired = (now - campaign.created_at) > (1 * 60 * 60); // 24 hours
        
        require!(is_expired && !campaign.is_funded, EscrowError::RefundNotAvailable);
        require!(!campaign.is_withdrawn, EscrowError::CampaignFundsWithdrawn);
        
        let campaign_account_info = campaign.to_account_info();
        require!(
            campaign_account_info.lamports() >= amount_to_refund,
            EscrowError::InsufficientEscrowBalance
        );

        **campaign_account_info.try_borrow_mut_lamports()? -= amount_to_refund;
        **ctx.accounts.user_wallet.try_borrow_mut_lamports()? += amount_to_refund;
        
        // We reload the campaign account to get the updated lamports balance.
        campaign.reload()?;
        campaign.amount_raised = campaign.to_account_info().lamports();

        Ok(())
    }

    // `withdraw_funds` is now signed by the admin.
    // It sends a specified amount to the whitelisted member and the rest to the fee wallet.
    pub fn withdraw_funds(ctx: Context<WithdrawFunds>, amount_to_whitelist: u64) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
    
        require!(campaign.is_funded, EscrowError::CampaignNotFunded);
        require!(!campaign.is_withdrawn, EscrowError::CampaignFundsWithdrawn);
       
            
        let campaign_account_info = campaign.to_account_info();
        let total_balance = campaign_account_info.lamports();
        
        require!(total_balance >= amount_to_whitelist, EscrowError::InsufficientEscrowBalance);
        **campaign_account_info.try_borrow_mut_lamports()? -= amount_to_whitelist;
        **ctx.accounts.whitelisted_member.try_borrow_mut_lamports()? += amount_to_whitelist;
      
        Ok(())
    }


    pub fn active_campaign(ctx: Context<ActiveCampaign>) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
        campaign.is_active = true; 
        Ok(())
    }
    // A function to reclaim SOL from an expired, unfunded campaign
    pub fn close_expired_campaign(ctx: Context<CloseExpiredCampaign>) -> Result<()> {
        let campaign = &ctx.accounts.campaign;
        let now = Clock::get()?.unix_timestamp;
        let is_expired = (now - campaign.created_at) > ( 1 * 60 * 60);

        require!(is_expired && !campaign.is_funded, EscrowError::CampaignNotExpiredOrFunded);
        // The `close` constraint will automatically transfer remaining lamports to the admin
        Ok(())
    }

    pub fn close_campaign(ctx: Context<CloseCampaign>) -> Result<()> {
        let campaign = &ctx.accounts.campaign;
        
        require!(!campaign.is_funded, EscrowError::CampaignNotExpiredOrFunded);
        // The `close` constraint will automatically transfer remaining lamports to the admin
        Ok(())
    }
}

// --- Enums and Account Structs ---

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy,Debug, PartialEq, Eq)]
pub enum CampaignType {
    Cto,
    DexBoost10,
    DexBoost30,
    DexBoost50,
    DexBoost100,
    DexBoost500,
    EnhancedTokenInfo,
}

#[account]
pub struct Config {
    pub admin: Pubkey,
    /// CHECK
    pub fee_wallet: Pubkey,
    pub bump: u8,
}

#[account]
pub struct Campaign {
    pub admin: Pubkey,
    pub token_to_update_mint: Pubkey,
    pub campaign_type: CampaignType,
    pub amount_raised: u64,
    pub goal_amount: u64,
    pub is_active: bool,
    pub is_funded: bool,
    pub is_withdrawn: bool,
    pub created_at: i64, // Unix Timestamp
    pub bump: u8,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 32 + 1,
        seeds = [b"config".as_ref()],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(campaign_type: CampaignType)]
pub struct CreateCampaign<'info> {
    /// CHECK: We only use this as a seed, no data is read.
    pub token_to_update_mint: AccountInfo<'info>,

    #[account(has_one = admin)]
    pub config: Account<'info, Config>,
    
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 32 + 1 + 8 + 8 + 32 + 1 + 1 + 1 + 8 + 1, // Recalculated space
        seeds = [
            b"campaign".as_ref(), 
            token_to_update_mint.key().as_ref(),
            match campaign_type {
                CampaignType::Cto => b"cto",
                CampaignType::DexBoost10 => b"dex_boost_10".as_ref(),
                CampaignType::DexBoost30 => b"dex_boost_30".as_ref(),
                CampaignType::DexBoost50 => b"dex_boost_50".as_ref(),
                CampaignType::DexBoost100 => b"dex_boost_100".as_ref(),
                CampaignType::DexBoost500 => b"dex_boost_500".as_ref(),
                CampaignType::EnhancedTokenInfo => b"token_info".as_ref(),
            }
            
        ],
        bump
    )]
    pub campaign: Account<'info, Campaign>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateCampaignStatus<'info> {
    #[account(mut, has_one = admin)]
    pub campaign: Account<'info, Campaign>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct ActiveCampaign<'info> {
    #[account(mut, has_one = admin)]
    pub campaign: Account<'info, Campaign>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct ProcessRefund<'info> {
    #[account(
        mut,
        seeds = [
            b"campaign".as_ref(), 
            campaign.token_to_update_mint.as_ref(),
            match campaign.campaign_type {
                CampaignType::Cto => b"cto",
                CampaignType::DexBoost10 => b"dex_boost_10".as_ref(),
                CampaignType::DexBoost30 => b"dex_boost_30".as_ref(),
                CampaignType::DexBoost50 => b"dex_boost_50".as_ref(),
                CampaignType::DexBoost100 => b"dex_boost_100".as_ref(),
                CampaignType::DexBoost500 => b"dex_boost_500".as_ref(),
                CampaignType::EnhancedTokenInfo => b"token_info".as_ref(),
            }
        ],
        bump = campaign.bump
    )]
    pub campaign: Account<'info, Campaign>,
    pub admin: Signer<'info>,
    
    /// CHECK: The user's wallet we are sending SOL back to.
    #[account(mut)]
    pub user_wallet: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawFunds<'info> {
    // Config account, used to get the fee_wallet address and verify the admin
    #[account(has_one = admin)]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [
            b"campaign".as_ref(), 
            campaign.token_to_update_mint.as_ref(),
            match campaign.campaign_type {
                CampaignType::Cto => b"cto",
                CampaignType::DexBoost10 => b"dex_boost_10".as_ref(),
                CampaignType::DexBoost30 => b"dex_boost_30".as_ref(),
                CampaignType::DexBoost50 => b"dex_boost_50".as_ref(),
                CampaignType::DexBoost100 => b"dex_boost_100".as_ref(),
                CampaignType::DexBoost500 => b"dex_boost_500".as_ref(),
                CampaignType::EnhancedTokenInfo => b"token_info".as_ref(),
            }
        ],
        bump = campaign.bump,
        close = fee_wallet 
    )]
    pub campaign: Account<'info, Campaign>,
    
    // The admin signer, verified by the `has_one` constraint on config
    pub admin: Signer<'info>,

    /// CHECK: The wallet of the whitelisted member to receive funds.
    #[account(mut)]
    pub whitelisted_member: AccountInfo<'info>,

    /// The fee wallet
    /// This is the destination for the remaining funds.
    /// CHECK
    #[account(mut)]
    pub fee_wallet: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseExpiredCampaign<'info> {
    #[account(
        mut,
        has_one = admin,
        seeds = [
            b"campaign".as_ref(), 
            campaign.token_to_update_mint.as_ref(),
            match campaign.campaign_type {
                CampaignType::Cto => b"cto",
                CampaignType::DexBoost10 => b"dex_boost_10".as_ref(),
                CampaignType::DexBoost30 => b"dex_boost_30".as_ref(),
                CampaignType::DexBoost50 => b"dex_boost_50".as_ref(),
                CampaignType::DexBoost100 => b"dex_boost_100".as_ref(),
                CampaignType::DexBoost500 => b"dex_boost_500".as_ref(),
                CampaignType::EnhancedTokenInfo => b"token_info".as_ref(),
            }
        ],
        bump = campaign.bump,
        close = admin // Send lamports to the admin who is closing it
    )]
    pub campaign: Account<'info, Campaign>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct CloseCampaign<'info> {
    #[account(
        mut,
        has_one = admin,
        seeds = [
            b"campaign".as_ref(), 
            campaign.token_to_update_mint.as_ref(),
            match campaign.campaign_type {
                CampaignType::Cto => b"cto",
                CampaignType::DexBoost10 => b"dex_boost_10".as_ref(),
                CampaignType::DexBoost30 => b"dex_boost_30".as_ref(),
                CampaignType::DexBoost50 => b"dex_boost_50".as_ref(),
                CampaignType::DexBoost100 => b"dex_boost_100".as_ref(),
                CampaignType::DexBoost500 => b"dex_boost_500".as_ref(),
                CampaignType::EnhancedTokenInfo => b"token_info".as_ref(),
            }
        ],
        bump = campaign.bump,
        close = admin // Send lamports to the admin who is closing it
    )]
    pub campaign: Account<'info, Campaign>,
    pub admin: Signer<'info>,
}

#[error_code]
pub enum EscrowError {
    #[msg("Campaign is not funded yet.")]
    CampaignNotFunded,
    #[msg("Campaign funds have already been withdrawn.")]
    CampaignFundsWithdrawn,
    #[msg("Provided member is not on the whitelist for this campaign.")]
    NotOnWhitelist,
    #[msg("Escrow has insufficient SOL balance for this operation.")]
    InsufficientEscrowBalance,
    #[msg("Refunds are not available for this campaign at this time.")]
    RefundNotAvailable,
    #[msg("Campaign is not expired or is already funded.")]
    CampaignNotExpiredOrFunded,
}