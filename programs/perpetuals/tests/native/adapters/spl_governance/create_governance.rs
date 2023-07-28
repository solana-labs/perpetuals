use {
    anchor_lang::prelude::Pubkey,
    perpetuals::adapters::spl_governance_program_adapter,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    spl_governance::state::{
        enums::{VoteThreshold, VoteTipping},
        governance::GovernanceConfig,
    },
    tokio::sync::RwLock,
};

#[allow(clippy::too_many_arguments)]
pub async fn create_governance(
    program_test_ctx: &RwLock<ProgramTestContext>,
    governing_token_owner: &Pubkey,
    create_authority: &Keypair,
    payer: &Keypair,
    realm_pda: &Pubkey,
    governing_token_mint: &Pubkey,
    community_vote_threshold: Option<VoteThreshold>,
    min_community_weight_to_create_proposal: Option<u64>,
    min_transaction_hold_up_time: Option<u32>,
    voting_base_time: Option<u32>,
    community_vote_tipping: Option<VoteTipping>,
) -> std::result::Result<(Pubkey, Pubkey), BanksClientError> {
    let token_owner_record_address =
        spl_governance::state::token_owner_record::get_token_owner_record_address(
            &spl_governance_program_adapter::id(),
            realm_pda,
            governing_token_mint,
            governing_token_owner,
        );

    let ix = spl_governance::instruction::create_governance(
        &spl_governance_program_adapter::id(),
        realm_pda,
        None,
        &token_owner_record_address,
        &payer.pubkey(),
        &create_authority.pubkey(),
        None,
        GovernanceConfig {
            community_vote_threshold: community_vote_threshold
                .unwrap_or(VoteThreshold::YesVotePercentage(50)),
            min_community_weight_to_create_proposal: min_community_weight_to_create_proposal
                .unwrap_or(1),
            min_transaction_hold_up_time: min_transaction_hold_up_time.unwrap_or(0),
            // 24 hours
            voting_base_time: voting_base_time.unwrap_or(3_600 * 24),
            community_vote_tipping: community_vote_tipping.unwrap_or(VoteTipping::Strict),

            // Disable council token
            council_vote_threshold: VoteThreshold::Disabled,
            council_veto_vote_threshold: VoteThreshold::Disabled,
            min_council_weight_to_create_proposal: 0,
            council_vote_tipping: VoteTipping::Disabled,
            community_veto_vote_threshold: VoteThreshold::Disabled,
            voting_cool_off_time: 0,
            deposit_exempt_proposal_count: 10,
        },
    );

    let governance_pda = ix.accounts[1].pubkey;
    let governed_account_address_pda = ix.accounts[2].pubkey;

    let mut ctx = program_test_ctx.write().await;
    let last_blockhash = ctx.last_blockhash;
    let banks_client = &mut ctx.banks_client;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, create_authority],
        last_blockhash,
    );

    banks_client.process_transaction(tx).await?;

    Ok((governance_pda, governed_account_address_pda))
}
