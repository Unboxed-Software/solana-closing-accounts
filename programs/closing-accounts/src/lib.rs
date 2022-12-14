use anchor_lang::prelude::*;
use anchor_lang::__private::CLOSED_ACCOUNT_DISCRIMINATOR;
use anchor_spl::{token::{TokenAccount, MintTo, Token, Mint, mint_to}};
use std::ops::DerefMut;

declare_id!("3v46VFd9BqVmnEK8TkT3iYVJPJpXget3jg4hMA1FbgZp");

#[program]
pub mod closing_accounts {
    use super::*;

    pub fn enter_lottery(ctx: Context<EnterLottery>) -> Result<()> {
        msg!("Initializing lottery entry...");

        ctx.accounts.lottery_entry.timestamp = Clock::get().unwrap().unix_timestamp;
        ctx.accounts.lottery_entry.user = ctx.accounts.user.key();
        ctx.accounts.lottery_entry.user_ata = ctx.accounts.user_ata.key();
        ctx.accounts.lottery_entry.bump = *ctx.bumps.get("lottery_entry").unwrap();

        msg!("Entry initialized!");

        Ok(())
    }

    pub fn redeem_winnings_insecure(ctx: Context<RedeemWinnings>) -> Result<()> {

        msg!("Calculating winnings");
        let amount = ctx.accounts.lottery_entry.timestamp as u64 * 10;

        msg!("Minting {} tokens in rewards", amount);
         // program signer seeds
        let auth_bump = *ctx.bumps.get("mint_auth").unwrap();
        let auth_seeds = &[MINT_SEED.as_bytes(), &[auth_bump]];
        let signer = &[&auth_seeds[..]];

        // donate RND by minting to vault
        mint_to(ctx.accounts.mint_ctx().with_signer(signer), amount)?;

        msg!("Closing account...");
        let account_to_close = ctx.accounts.lottery_entry.to_account_info();
        let dest_starting_lamports = ctx.accounts.user.lamports();

        **ctx.accounts.user.lamports.borrow_mut() = dest_starting_lamports
            .checked_add(account_to_close.lamports())
            .unwrap();
        **account_to_close.lamports.borrow_mut() = 0;

        let mut data = account_to_close.try_borrow_mut_data()?;
        for byte in data.deref_mut().iter_mut() {
            *byte = 0;
        }

        msg!("Lottery lamports: {:?}", account_to_close.lamports);
        msg!("Lottery account closed");

        Ok(())
    }

    pub fn force_defund(ctx: Context<ForceDefund>) -> Result<()> {
        let account = &ctx.accounts.data_account;

        let data = account.try_borrow_data()?;
        assert!(data.len() > 8);

        let mut discriminator = [0u8; 8];
        discriminator.copy_from_slice(&data[0..8]);
        if discriminator != CLOSED_ACCOUNT_DISCRIMINATOR {
            return err!(MyError::InvalidDiscriminator);
        }

        let dest_starting_lamports = ctx.accounts.destination.lamports();

        **ctx.accounts.destination.lamports.borrow_mut() = dest_starting_lamports
            .checked_add(account.lamports())
            .unwrap();
        **account.lamports.borrow_mut() = 0;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct EnterLottery<'info> {
    #[account(
        init,
        seeds = [DATA_PDA_SEED.as_bytes(), user.key().as_ref()],
        bump,
        payer = user,
        space = 8 + 1 + 32 + 1 + 8 + 32
    )]
    pub lottery_entry: Account<'info, LotteryAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub user_ata: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>
}

#[derive(Accounts)]
pub struct RedeemWinnings<'info> {
    // program expects this account to be initialized
    #[account(
        mut,
        seeds = [DATA_PDA_SEED.as_bytes(), user.key().as_ref()],
        bump = lottery_entry.bump,
        has_one = user
    )]
    pub lottery_entry: Account<'info, LotteryAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        constraint = user_ata.key() == lottery_entry.user_ata
    )]
    pub user_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = reward_mint.key() == user_ata.mint
    )]
    pub reward_mint: Account<'info, Mint>,
    ///CHECK: mint authority
    #[account(
        seeds = [MINT_SEED.as_bytes()],
        bump
    )]
    pub mint_auth: AccountInfo<'info>,
    pub token_program: Program<'info, Token>
}

#[derive(Accounts)]
pub struct ForceDefund<'info> {
    ///CHECK: safe
    #[account(mut)]
    data_account: AccountInfo<'info>,
    ///CHECK: safe
    #[account(mut)]
    destination: AccountInfo<'info>,
}

#[account]
pub struct LotteryAccount {
    is_initialized: bool,
    user: Pubkey,
    bump: u8,
    timestamp: i64,
    user_ata: Pubkey
}

pub const DATA_PDA_SEED: &str = "test-seed";
pub const MINT_SEED: &str = "mint-seed";

impl<'info> RedeemWinnings <'info> {
    pub fn mint_ctx(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_program = self.token_program.to_account_info();
        let cpi_accounts = MintTo {
            mint: self.reward_mint.to_account_info(),
            to: self.user_ata.to_account_info(),
            authority: self.mint_auth.to_account_info()
        };

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

#[error_code]
pub enum MyError {
    #[msg("Expected closed account discriminator")]
    InvalidDiscriminator
}