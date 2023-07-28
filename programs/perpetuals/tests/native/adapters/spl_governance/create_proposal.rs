use {
    anchor_lang::prelude::Pubkey,
    perpetuals::adapters::spl_governance_program_adapter,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    spl_governance::state::proposal::VoteType,
    tokio::sync::RwLock,
};

#[allow(clippy::too_many_arguments)]
pub async fn create_proposal(
    program_test_ctx: &RwLock<ProgramTestContext>,
    payer: &Keypair,
    name: String,
    description_link: String,
    realm_pda: &Pubkey,
    governance_pda: &Pubkey,
    governing_token_mint: &Pubkey,
    governing_token_owner: &Pubkey,
    governance_authority: &Keypair,
) -> std::result::Result<Pubkey, BanksClientError> {
    let proposal_owner_record =
        spl_governance::state::token_owner_record::get_token_owner_record_address(
            &spl_governance_program_adapter::id(),
            realm_pda,
            governing_token_mint,
            governing_token_owner,
        );

    // Create the proposal
    let (proposal_pda, create_proposal_ix) = {
        let proposal_seed = Pubkey::new_unique();

        let ix = spl_governance::instruction::create_proposal(
            &spl_governance_program_adapter::id(),
            governance_pda,
            &proposal_owner_record,
            &governance_authority.pubkey(),
            &payer.pubkey(),
            None,
            realm_pda,
            name,
            description_link,
            governing_token_mint,
            VoteType::SingleChoice,
            vec!["Yes".to_string()],
            false,
            &proposal_seed,
        );

        let proposal_pda = ix.accounts[1].pubkey;

        (proposal_pda, ix)
    };

    // Add signatory (governance_authority)
    let add_signatory_ix = spl_governance::instruction::add_signatory(
        &spl_governance_program_adapter::id(),
        &proposal_pda,
        &proposal_owner_record,
        &governance_authority.pubkey(),
        &payer.pubkey(),
        &governance_authority.pubkey(),
    );

    // Sign-off proposal
    let sign_off_proposal_ix = spl_governance::instruction::sign_off_proposal(
        &spl_governance_program_adapter::id(),
        realm_pda,
        governance_pda,
        &proposal_pda,
        &governance_authority.pubkey(),
        None,
    );

    let mut ctx = program_test_ctx.write().await;
    let last_blockhash = ctx.last_blockhash;
    let banks_client = &mut ctx.banks_client;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[create_proposal_ix, add_signatory_ix, sign_off_proposal_ix],
        Some(&payer.pubkey()),
        &[payer, governance_authority],
        last_blockhash,
    );

    banks_client.process_transaction(tx).await?;

    Ok(proposal_pda)
}
