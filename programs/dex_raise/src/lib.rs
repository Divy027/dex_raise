
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("7F6rVPdLuDi15Mdoq57wwEnNPQnshfW6z7gDYHoQP3o3");

const USDC_MINT_PUBKEY: &str = "Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr"; // Mainnet USDC Mint
const MINIMUM_DONATION_LAMPORTS: u64 = 5_000_000;

#[program]
pub mod dexscreener_escrow {
    use super::*;

    // Initializes the global configuration. Should only be called once.
    pub fn initialize(
        ctx: Context<Initialize>,
        fee_basis_points: u16,
        initial_whitelisted_member: Pubkey,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.admin = ctx.accounts.admin.key();
        config.treasury = ctx.accounts.treasury.key();
        config.fee_basis_points = fee_basis_points;
        config.whitelist = vec![initial_whitelisted_member];
        config.bump = ctx.bumps.config;
        Ok(())
    }

    // Admin function to add a member to the withdrawal whitelist.
    pub fn add_to_whitelist(ctx: Context<UpdateWhitelist>, member: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(!config.whitelist.contains(&member), EscrowError::AlreadyOnWhitelist);
        config.whitelist.push(member);
        Ok(())
    }

    // Admin function to remove a member from the withdrawal whitelist.
    pub fn remove_from_whitelist(ctx: Context<UpdateWhitelist>, member: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.whitelist.retain(|&x| x != member);
        Ok(())
    }

    // to create a new fundraising campaign.
    pub fn create_campaign(ctx: Context<CreateCampaign>, goal_amount: u64, creator: Pubkey) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
        campaign.creator = creator;
        campaign.admin = ctx.accounts.admin.key(); 
        campaign.token_to_update_mint = ctx.accounts.token_to_update_mint.to_account_info().key();
        campaign.usdc_mint = ctx.accounts.usdc_mint.key();
        campaign.amount_raised = 0;
        campaign.goal_amount = goal_amount;
        campaign.is_active = true;
        campaign.is_funded = false;
        campaign.is_withdrawn = false;
        campaign.bump = ctx.bumps.campaign;
        campaign.escrow_vault_bump = ctx.bumps.escrow_vault;

        let usdc_mint_pubkey: Pubkey = USDC_MINT_PUBKEY.parse().unwrap();
        require!(campaign.usdc_mint == usdc_mint_pubkey, EscrowError::InvalidUsdcMint);
        Ok(())
    }

    // This function doesn't move any tokens. It just updates the state.
    pub fn record_donation(ctx: Context<RecordDonation>, amount: u64) -> Result<()> {
        let campaign = &mut ctx.accounts.campaign;
        require!(campaign.is_active, EscrowError::CampaignNotActive);
        require!(!campaign.is_funded, EscrowError::CampaignAlreadyFunded);

        campaign.amount_raised = campaign.amount_raised.checked_add(amount).unwrap();

        if campaign.amount_raised >= campaign.goal_amount {
            campaign.is_funded = true;
            campaign.is_active = false;
        }
        
        Ok(())
    }

   

   // `process_refund` is an admin-only function that sends funds TO a user
   pub fn process_refund(ctx: Context<ProcessRefund>, amount_to_refund: u64) -> Result<()> {

        require!(!ctx.accounts.campaign.is_withdrawn, EscrowError::CampaignFundsWithdrawn);
        let campaign_key = ctx.accounts.campaign.key();
        let fee_basis_points = ctx.accounts.config.fee_basis_points as u64;
       
        let processing_fee = (amount_to_refund * fee_basis_points) / 10000;
        let refund_amount = amount_to_refund.checked_sub(processing_fee).unwrap();

        require!(
            ctx.accounts.escrow_vault.amount >= amount_to_refund,
            EscrowError::InsufficientVaultBalance
        );

        let campaign_key = campaign_key;
        let bump_seed = ctx.accounts.campaign.bump.to_le_bytes();
        let seeds = &[
            b"campaign".as_ref(),                                 
            ctx.accounts.campaign.token_to_update_mint.as_ref(), 
            &bump_seed,        
        ];
        let signer = &[&seeds[..]];

        token::transfer(
            ctx.accounts.transfer_to_user_context().with_signer(signer),
            refund_amount,
        )?;

        // Transfer fee to treasury
        token::transfer(
            ctx.accounts.transfer_to_treasury_context().with_signer(signer),
            processing_fee,
        )?;

        let campaign = &mut ctx.accounts.campaign;
        campaign.amount_raised = campaign.amount_raised.checked_sub(amount_to_refund).unwrap();

        if campaign.amount_raised < campaign.goal_amount {
            campaign.is_funded = false;
            campaign.is_active = true;
        }

        Ok(())
    }

    // `withdraw_funds` is now signed by the admin, sending funds to a whitelisted member
    pub fn withdraw_funds(ctx: Context<WithdrawFunds>) -> Result<()> {
      
        let config = &ctx.accounts.config;

        require!(ctx.accounts.campaign.is_funded, EscrowError::CampaignNotFunded);
        require!(!ctx.accounts.campaign.is_withdrawn, EscrowError::CampaignFundsWithdrawn);
        require!(
            config.whitelist.contains(&ctx.accounts.member_usdc_account.owner.key()),
            EscrowError::NotOnWhitelist
        );

        let campaign_key = ctx.accounts.campaign.key();
        let bump_seed = ctx.accounts.campaign.bump.to_le_bytes();
        let seeds = &[
            b"campaign".as_ref(),                                 
            ctx.accounts.campaign.token_to_update_mint.as_ref(), 
            &bump_seed,        
        ];
        let signer = &[&seeds[..]];

        token::transfer(
            ctx.accounts.transfer_to_member_context().with_signer(signer),
            ctx.accounts.campaign.goal_amount,
        )?;
        let campaign = &mut ctx.accounts.campaign;

        campaign.is_withdrawn = true;
        campaign.is_active = false;

        Ok(())
    }

    pub fn sweep_sol(ctx: Context<SweepSol>) -> Result<()> {
        let source_account = &ctx.accounts.source_account;
        let destination_account = &ctx.accounts.destination_account;
        
        let amount_to_sweep = source_account.lamports();
    
        // Check if there's anything to sweep
        if amount_to_sweep == 0 {
            return Ok(()); // Nothing to do
        }
        
        // Debit lamports from the source
        **source_account.try_borrow_mut_lamports()? -= amount_to_sweep;
        // Credit lamports to the destination
        **destination_account.try_borrow_mut_lamports()? += amount_to_sweep;
        
        Ok(())
    }
}

// -----------------
// Account Structs & Contexts
// -----------------

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 32 + 2 + 4 + (32 * 10) + 1, // Space for admin, treasury, fee, whitelist (10 members), bump
        seeds = [b"config".as_ref()],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub admin: Signer<'info>,
    /// CHECK: The treasury address doesn't need to be checked, it's just a pubkey.
    pub treasury: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateWhitelist<'info> {
    #[account(
        mut,
        has_one = admin,
        seeds = [b"config".as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(goal_amount: u64, creator: Pubkey)]
pub struct CreateCampaign<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 32 + 32 + 32 + 8 + 8 + 1 + 1 + 1 + 1 + 1, // Added space for admin pubkey
        seeds = [b"campaign".as_ref(), token_to_update_mint.key().as_ref()],
        bump
    )]
    pub campaign: Account<'info, Campaign>,

    #[account(
        init, // This vault is created along with the campaign
        payer = admin,
        token::mint = usdc_mint,
        token::authority = campaign,
        seeds = [b"escrow_vault".as_ref(), campaign.key().as_ref()],
        bump
    )]
    pub escrow_vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    pub token_to_update_mint: Account<'info, Mint>,
    #[account(address = USDC_MINT_PUBKEY.parse::<Pubkey>().unwrap() @ EscrowError::InvalidUsdcMint)]
    pub usdc_mint: Account<'info, Mint>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct RecordDonation<'info> {
    #[account(mut, has_one = admin)]
    pub campaign: Account<'info, Campaign>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct ProcessRefund<'info> {
    #[account(mut, has_one = admin)]
    pub campaign: Account<'info, Campaign>,
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"escrow_vault".as_ref(), campaign.key().as_ref()],
        bump
    )]
    pub escrow_vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub user_usdc_account: Account<'info, TokenAccount>, // The account to refund TO

    #[account(mut, address = config.treasury @ EscrowError::InvalidTreasury)]
    /// CHECK: Treasury is just a pubkey checked against the config.
    pub treasury_account: AccountInfo<'info>,

    #[account(seeds = [b"config".as_ref()], bump = config.bump)]
    pub config: Account<'info, Config>,
    
    pub token_program: Program<'info, Token>,
}

impl<'info> ProcessRefund<'info> {
    fn transfer_to_user_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.escrow_vault.to_account_info(),
                to: self.user_usdc_account.to_account_info(),
                authority: self.campaign.to_account_info(),
            },
        )
    }

    fn transfer_to_treasury_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        CpiContext::new(
            self.token_program.to_account_info(),
            Transfer {
                from: self.escrow_vault.to_account_info(),
                to: self.treasury_account.to_account_info(),
                authority: self.campaign.to_account_info(),
            },
        )
    }
}


#[derive(Accounts)]
pub struct WithdrawFunds<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"campaign".as_ref(), campaign.token_to_update_mint.as_ref()],
        has_one = admin,
        bump 
    )]
    pub campaign: Account<'info, Campaign>,
    
    #[account(
        mut,
        seeds = [b"escrow_vault".as_ref(), campaign.key().as_ref()],
        bump = campaign.escrow_vault_bump 
    )]
    pub escrow_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub member_usdc_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"config".as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,
    pub token_program: Program<'info, Token>,
}


impl<'info> WithdrawFunds<'info> {
    fn transfer_to_member_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.escrow_vault.to_account_info(),
            to: self.member_usdc_account.to_account_info(),
            authority: self.campaign.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct SweepSol<'info> {
    #[account(mut, has_one = admin)]
    pub config: Account<'info, Config>,
    pub admin: Signer<'info>,

    /// The account to sweep SOL from. Can be a campaign PDA or even an escrow vault PDA.
    /// CHECK: This is safe because we are only transferring lamports and the instruction is admin-only.
    /// The `mut` constraint ensures we can debit the lamports.
    #[account(mut)]
    pub source_account: AccountInfo<'info>,
    
    /// The account to send the SOL to (e.g., the treasury).
    #[account(mut)]
    /// CHECK: This is safe because we are only transferring lamports and the instruction is admin-only.
    pub destination_account: AccountInfo<'info>,
}


// -----------------
// On-Chain State Accounts
// -----------------

#[account]
pub struct Config {
    pub admin: Pubkey,
    pub treasury: Pubkey,
    pub fee_basis_points: u16, // e.g., 50 for 0.5%
    pub whitelist: Vec<Pubkey>,
    pub bump: u8,
}

#[account]
pub struct Campaign {
    pub creator: Pubkey, // Original user who requested it
    pub admin: Pubkey,   // Admin who controls this campaign
    pub token_to_update_mint: Pubkey,
    pub usdc_mint: Pubkey,
    pub amount_raised: u64,
    pub goal_amount: u64,
    pub is_active: bool,
    pub is_funded: bool,
    pub is_withdrawn: bool,
    pub bump: u8,
    pub escrow_vault_bump: u8, 
}


// -----------------
// Error Codes
// -----------------

#[error_code]
pub enum EscrowError {
    #[msg("This campaign is not currently active.")]
    CampaignNotActive,
    #[msg("This campaign has already been fully funded.")]
    CampaignAlreadyFunded,
    #[msg("This campaign's funds have already been withdrawn.")]
    CampaignFundsWithdrawn,
    #[msg("The campaign has not yet reached its funding goal.")]
    CampaignNotFunded,
    #[msg("The provided address is not on the withdrawal whitelist.")]
    NotOnWhitelist,
    #[msg("The provided address is already on the whitelist.")]
    AlreadyOnWhitelist,
    #[msg("Donation amount is below the minimum required.")]
    DonationTooSmall,
    #[msg("No donation record was found for this user and campaign.")]
    NoDonationFound,
    #[msg("The provided USDC mint is not the official one.")]
    InvalidUsdcMint,
    #[msg("The provided treasury account is incorrect.")]
    InvalidTreasury,
    #[msg("The vault has insufficient funds for this operation.")]
    InsufficientVaultBalance,
}