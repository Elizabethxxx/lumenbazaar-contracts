extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    vec, Address, BytesN, Env,
};
use upto_session::{AssetPolicy, ContractError, UptoSessionContract, UptoSessionContractClient};

fn initialized_wallet(
    env: &Env,
) -> (
    PolicyWalletExampleContractClient<'_>,
    Address,
    Address,
    Address,
    Address,
    BytesN<32>,
) {
    let contract_id = env.register(PolicyWalletExampleContract, ());
    let client = PolicyWalletExampleContractClient::new(env, &contract_id);
    let owner = Address::generate(env);
    let agent = Address::generate(env);
    let seller = Address::generate(env);
    let asset = Address::generate(env);
    let resource_hash = BytesN::from_array(env, &[12; 32]);

    client.initialize(&owner, &Address::generate(env));

    (client, agent, seller, asset, owner, resource_hash)
}

fn set_standard_policy(
    env: &Env,
    client: &PolicyWalletExampleContractClient<'_>,
    agent: &Address,
    seller: &Address,
    asset: &Address,
    resource_hash: &BytesN<32>,
) {
    client.set_policy(
        &standard_policy(env, agent, seller, asset, resource_hash, 100),
        &0,
    );
}

fn standard_policy(
    env: &Env,
    agent: &Address,
    seller: &Address,
    asset: &Address,
    resource_hash: &BytesN<32>,
    valid_until_ledger: u32,
) -> SpendingPolicy {
    SpendingPolicy {
        agent: agent.clone(),
        max_amount_per_payment: 75,
        max_amount_per_day: 100,
        valid_until_ledger,
        allowed_sellers: vec![env, seller.clone()],
        allowed_assets: vec![env, asset.clone()],
        allowed_resource_hashes: vec![env, resource_hash.clone()],
        revision: 0,
    }
}

#[test]
fn policy_wallet_example_is_registered_independently() {
    let env = Env::default();
    let contract_id = env.register(PolicyWalletExampleContract, ());
    let client = PolicyWalletExampleContractClient::new(&env, &contract_id);

    assert!(client.example_only());
}

#[test]
fn initializes_config_once_with_owner_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PolicyWalletExampleContract, ());
    let client = PolicyWalletExampleContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let upto_session_contract = Address::generate(&env);

    client.initialize(&owner, &upto_session_contract);

    let config = client.get_config();
    assert_eq!(config.owner, owner);
    assert_eq!(config.upto_session_contract, upto_session_contract);
    assert_eq!(config.initialized_at_ledger, env.ledger().sequence());
    assert!(config.example_only);
    assert_eq!(
        client.try_initialize(&owner, &upto_session_contract),
        Err(Ok(PolicyWalletError::AlreadyInitialized))
    );
}

#[test]
fn initialize_requires_owner_auth() {
    let env = Env::default();
    let contract_id = env.register(PolicyWalletExampleContract, ());
    let client = PolicyWalletExampleContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let upto_session_contract = Address::generate(&env);

    assert!(client
        .try_initialize(&owner, &upto_session_contract)
        .is_err());
    assert_eq!(
        client.try_get_config(),
        Err(Ok(PolicyWalletError::NotInitialized))
    );
}

#[test]
fn require_owner_uses_stored_owner_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PolicyWalletExampleContract, ());
    let client = PolicyWalletExampleContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let upto_session_contract = Address::generate(&env);

    client.initialize(&owner, &upto_session_contract);

    assert!(client.try_require_owner().is_ok());
}

#[test]
fn policy_wallet_storage_is_isolated_from_upto_session() {
    let env = Env::default();
    env.mock_all_auths();
    let wallet_id = env.register(PolicyWalletExampleContract, ());
    let wallet_client = PolicyWalletExampleContractClient::new(&env, &wallet_id);
    let upto_session_id = env.register(UptoSessionContract, ());
    let upto_client = UptoSessionContractClient::new(&env, &upto_session_id);
    let owner = Address::generate(&env);
    let asset = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let missing_session_id = BytesN::from_array(&env, &[9; 32]);

    wallet_client.initialize(&owner, &upto_session_id);

    assert_eq!(
        upto_client.try_get_session(&missing_session_id),
        Err(Ok(ContractError::SessionNotFound))
    );
    assert!(upto_client
        .try_initialize(
            &owner,
            &soroban_sdk::vec![
                &env,
                AssetPolicy {
                    asset,
                    max_session_amount: 1,
                }
            ]
        )
        .is_ok());
}

#[test]
fn owner_sets_spending_policy() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    set_standard_policy(&env, &client, &agent, &seller, &asset, &resource_hash);

    let policy = client.get_policy(&agent);
    assert_eq!(policy.agent, agent);
    assert_eq!(policy.max_amount_per_payment, 75);
    assert_eq!(policy.max_amount_per_day, 100);
    assert_eq!(policy.valid_until_ledger, 100);
    assert_eq!(policy.allowed_sellers, vec![&env, seller]);
    assert_eq!(policy.allowed_assets, vec![&env, asset]);
    assert_eq!(policy.allowed_resource_hashes, vec![&env, resource_hash]);
    assert_eq!(policy.revision, 1);
}

#[test]
fn check_payment_accepts_policy_match_without_recording_spend() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);
    set_standard_policy(&env, &client, &agent, &seller, &asset, &resource_hash);

    assert!(client
        .try_check_payment(&agent, &seller, &asset, &50, &resource_hash)
        .is_ok());
    assert_eq!(client.spent_today(&agent), 0);
}

#[test]
fn authorize_payment_records_daily_spend() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);
    set_standard_policy(&env, &client, &agent, &seller, &asset, &resource_hash);

    client.authorize_payment(&agent, &seller, &asset, &60, &resource_hash);
    client.authorize_payment(&agent, &seller, &asset, &40, &resource_hash);

    assert_eq!(client.spent_today(&agent), 100);
}

#[test]
fn rejects_amounts_over_payment_or_daily_caps() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);
    set_standard_policy(&env, &client, &agent, &seller, &asset, &resource_hash);

    assert_eq!(
        client.try_check_payment(&agent, &seller, &asset, &76, &resource_hash),
        Err(Ok(PolicyWalletError::AmountExceedsPaymentCap))
    );

    client.authorize_payment(&agent, &seller, &asset, &75, &resource_hash);
    assert_eq!(
        client.try_authorize_payment(&agent, &seller, &asset, &26, &resource_hash),
        Err(Ok(PolicyWalletError::AmountExceedsDailyCap))
    );
}

#[test]
fn rejects_disallowed_seller_asset_or_resource_hash() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);
    let wrong_seller = Address::generate(&env);
    let wrong_asset = Address::generate(&env);
    let wrong_resource_hash = BytesN::from_array(&env, &[13; 32]);
    set_standard_policy(&env, &client, &agent, &seller, &asset, &resource_hash);

    assert_eq!(
        client.try_check_payment(&agent, &wrong_seller, &asset, &50, &resource_hash),
        Err(Ok(PolicyWalletError::SellerNotAllowed))
    );
    assert_eq!(
        client.try_check_payment(&agent, &seller, &wrong_asset, &50, &resource_hash),
        Err(Ok(PolicyWalletError::AssetNotAllowed))
    );
    assert_eq!(
        client.try_check_payment(&agent, &seller, &asset, &50, &wrong_resource_hash),
        Err(Ok(PolicyWalletError::ResourceHashNotAllowed))
    );
}

#[test]
fn rejects_invalid_or_expired_policy_windows() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    assert_eq!(
        client.try_set_policy(
            &SpendingPolicy {
                agent: agent.clone(),
                max_amount_per_payment: 0,
                max_amount_per_day: 100,
                valid_until_ledger: 100,
                allowed_sellers: vec![&env, seller.clone()],
                allowed_assets: vec![&env, asset.clone()],
                allowed_resource_hashes: vec![&env, resource_hash.clone()],
                revision: 0,
            },
            &0
        ),
        Err(Ok(PolicyWalletError::InvalidPolicy))
    );
    assert_eq!(
        client.try_set_policy(
            &SpendingPolicy {
                agent,
                max_amount_per_payment: 10,
                max_amount_per_day: 100,
                valid_until_ledger: 0,
                allowed_sellers: vec![&env, seller],
                allowed_assets: vec![&env, asset],
                allowed_resource_hashes: vec![&env, resource_hash],
                revision: 0,
            },
            &0
        ),
        Err(Ok(PolicyWalletError::PolicyExpired))
    );
}

#[test]
fn documentation_allowed_payment_example_authorizes() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);
    set_standard_policy(&env, &client, &agent, &seller, &asset, &resource_hash);

    assert!(client
        .try_authorize_payment(&agent, &seller, &asset, &50, &resource_hash)
        .is_ok());
    assert_eq!(client.spent_today(&agent), 50);
}

#[test]
fn documentation_blocked_payment_example_reports_amount_cap() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);
    set_standard_policy(&env, &client, &agent, &seller, &asset, &resource_hash);

    assert_eq!(
        client.try_check_payment(&agent, &seller, &asset, &76, &resource_hash),
        Err(Ok(PolicyWalletError::AmountExceedsPaymentCap))
    );
}

#[test]
fn documentation_expired_policy_example_reports_expired() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);
    client.set_policy(
        &standard_policy(&env, &agent, &seller, &asset, &resource_hash, 10),
        &0,
    );

    env.ledger().set_sequence_number(11);

    assert_eq!(
        client.try_check_payment(&agent, &seller, &asset, &50, &resource_hash),
        Err(Ok(PolicyWalletError::PolicyExpired))
    );
}

#[test]
fn documentation_wrong_seller_example_reports_seller_error() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);
    let wrong_seller = Address::generate(&env);
    set_standard_policy(&env, &client, &agent, &seller, &asset, &resource_hash);

    assert_eq!(
        client.try_check_payment(&agent, &wrong_seller, &asset, &50, &resource_hash),
        Err(Ok(PolicyWalletError::SellerNotAllowed))
    );
}

#[test]
fn documentation_wrong_asset_example_reports_asset_error() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);
    let wrong_asset = Address::generate(&env);
    set_standard_policy(&env, &client, &agent, &seller, &asset, &resource_hash);

    assert_eq!(
        client.try_check_payment(&agent, &seller, &wrong_asset, &50, &resource_hash),
        Err(Ok(PolicyWalletError::AssetNotAllowed))
    );
}

#[test]
fn policy_creation_and_update_follow_deterministic_revisions() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    // Initial policy creation with expected_revision 0
    let mut policy = standard_policy(&env, &agent, &seller, &asset, &resource_hash, 100);
    client.set_policy(&policy, &0);

    let stored = client.get_policy(&agent);
    assert_eq!(stored.revision, 1);
    assert_eq!(client.get_policy_revision(&agent), 1);

    // Update with expected_revision 1 advances to 2
    policy.max_amount_per_payment = 80;
    client.set_policy(&policy, &1);

    let updated = client.get_policy(&agent);
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.max_amount_per_payment, 80);
    assert_eq!(client.get_policy_revision(&agent), 2);
}

#[test]
fn stale_expected_revision_fails_without_modifying_policy_or_spend() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    let mut policy = standard_policy(&env, &agent, &seller, &asset, &resource_hash, 100);
    client.set_policy(&policy, &0);

    // Authorize payment to set spend state
    client.authorize_payment(&agent, &seller, &asset, &20, &resource_hash);
    assert_eq!(client.spent_today(&agent), 20);

    // Attempt stale write with expected_revision 0
    policy.max_amount_per_payment = 90;
    let res = client.try_set_policy(&policy, &0);
    assert_eq!(res, Err(Ok(PolicyWalletError::StaleRevision)));

    // Policy and spend remain unchanged
    let stored = client.get_policy(&agent);
    assert_eq!(stored.revision, 1);
    assert_eq!(stored.max_amount_per_payment, 75);
    assert_eq!(client.spent_today(&agent), 20);

    // Attempt future stale revision (e.g. 99)
    let res_future = client.try_set_policy(&policy, &99);
    assert_eq!(res_future, Err(Ok(PolicyWalletError::StaleRevision)));
    assert_eq!(client.get_policy(&agent).revision, 1);
}

#[test]
fn concurrent_tooling_conflicts_are_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    let policy_init = standard_policy(&env, &agent, &seller, &asset, &resource_hash, 100);
    client.set_policy(&policy_init, &0);

    // Both Tooling A and Tooling B observe revision 1
    let rev_a = client.get_policy_revision(&agent);
    let rev_b = client.get_policy_revision(&agent);
    assert_eq!(rev_a, 1);
    assert_eq!(rev_b, 1);

    // Tooling A updates first
    let mut policy_a = policy_init.clone();
    policy_a.max_amount_per_payment = 60;
    assert!(client.try_set_policy(&policy_a, &rev_a).is_ok());

    // Tooling B tries to update using its stale view (revision 1)
    let mut policy_b = policy_init.clone();
    policy_b.max_amount_per_payment = 70;
    assert_eq!(
        client.try_set_policy(&policy_b, &rev_b),
        Err(Ok(PolicyWalletError::StaleRevision))
    );

    // Tooling A's policy is active
    let stored = client.get_policy(&agent);
    assert_eq!(stored.revision, 2);
    assert_eq!(stored.max_amount_per_payment, 60);
}

#[test]
fn independent_agents_have_isolated_policy_revisions() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent1, seller, asset, _owner, resource_hash) = initialized_wallet(&env);
    let agent2 = Address::generate(&env);

    let policy1 = standard_policy(&env, &agent1, &seller, &asset, &resource_hash, 100);
    let policy2 = standard_policy(&env, &agent2, &seller, &asset, &resource_hash, 100);

    client.set_policy(&policy1, &0);
    client.set_policy(&policy2, &0);

    assert_eq!(client.get_policy_revision(&agent1), 1);
    assert_eq!(client.get_policy_revision(&agent2), 1);

    // Update agent1 multiple times
    let mut p1_update = policy1.clone();
    p1_update.max_amount_per_payment = 50;
    client.set_policy(&p1_update, &1);
    p1_update.max_amount_per_payment = 40;
    client.set_policy(&p1_update, &2);

    assert_eq!(client.get_policy_revision(&agent1), 3);
    assert_eq!(client.get_policy_revision(&agent2), 1);
}

#[test]
fn distinct_create_path_creates_policy_and_rejects_existing() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    let policy = standard_policy(&env, &agent, &seller, &asset, &resource_hash, 100);
    assert!(client.try_create_policy(&policy).is_ok());
    assert_eq!(client.get_policy_revision(&agent), 1);

    // Calling create_policy again on existing policy fails with StaleRevision
    assert_eq!(
        client.try_create_policy(&policy),
        Err(Ok(PolicyWalletError::StaleRevision))
    );
}

#[test]
fn allowlists_enforce_exact_limits_and_fail_at_limit_plus_one() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, _seller, _asset, _owner, _resource_hash) = initialized_wallet(&env);

    // Exact limits (16 items each) pass
    let mut sellers_max = vec![&env];
    for _ in 0..MAX_ALLOWED_SELLERS {
        sellers_max.push_back(Address::generate(&env));
    }
    let mut assets_max = vec![&env];
    for _ in 0..MAX_ALLOWED_ASSETS {
        assets_max.push_back(Address::generate(&env));
    }
    let mut hashes_max = vec![&env];
    for i in 0..MAX_ALLOWED_RESOURCE_HASHES {
        hashes_max.push_back(BytesN::from_array(&env, &[(i + 1) as u8; 32]));
    }

    let exact_policy = SpendingPolicy {
        agent: agent.clone(),
        max_amount_per_payment: 50,
        max_amount_per_day: 100,
        valid_until_ledger: 100,
        allowed_sellers: sellers_max.clone(),
        allowed_assets: assets_max.clone(),
        allowed_resource_hashes: hashes_max.clone(),
        revision: 0,
    };
    assert!(client.try_set_policy(&exact_policy, &0).is_ok());

    let agent_oversized = Address::generate(&env);

    // Sellers limit + 1 fails
    let mut sellers_oversized = sellers_max.clone();
    sellers_oversized.push_back(Address::generate(&env));
    let mut p_sellers_over = exact_policy.clone();
    p_sellers_over.agent = agent_oversized.clone();
    p_sellers_over.allowed_sellers = sellers_oversized;
    assert_eq!(
        client.try_set_policy(&p_sellers_over, &0),
        Err(Ok(PolicyWalletError::InvalidPolicy))
    );

    // Assets limit + 1 fails
    let mut assets_oversized = assets_max.clone();
    assets_oversized.push_back(Address::generate(&env));
    let mut p_assets_over = exact_policy.clone();
    p_assets_over.agent = agent_oversized.clone();
    p_assets_over.allowed_assets = assets_oversized;
    assert_eq!(
        client.try_set_policy(&p_assets_over, &0),
        Err(Ok(PolicyWalletError::InvalidPolicy))
    );

    // Hashes limit + 1 fails
    let mut hashes_oversized = hashes_max.clone();
    hashes_oversized.push_back(BytesN::from_array(&env, &[99; 32]));
    let mut p_hashes_over = exact_policy.clone();
    p_hashes_over.agent = agent_oversized.clone();
    p_hashes_over.allowed_resource_hashes = hashes_oversized;
    assert_eq!(
        client.try_set_policy(&p_hashes_over, &0),
        Err(Ok(PolicyWalletError::InvalidPolicy))
    );
}

#[test]
fn allowlists_reject_duplicate_sellers_deterministically() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    let dup_policy = SpendingPolicy {
        agent,
        max_amount_per_payment: 50,
        max_amount_per_day: 100,
        valid_until_ledger: 100,
        allowed_sellers: vec![&env, seller.clone(), seller],
        allowed_assets: vec![&env, asset],
        allowed_resource_hashes: vec![&env, resource_hash],
        revision: 0,
    };
    assert_eq!(
        client.try_set_policy(&dup_policy, &0),
        Err(Ok(PolicyWalletError::InvalidPolicy))
    );
}

#[test]
fn allowlists_reject_duplicate_assets_deterministically() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    let dup_policy = SpendingPolicy {
        agent,
        max_amount_per_payment: 50,
        max_amount_per_day: 100,
        valid_until_ledger: 100,
        allowed_sellers: vec![&env, seller],
        allowed_assets: vec![&env, asset.clone(), asset],
        allowed_resource_hashes: vec![&env, resource_hash],
        revision: 0,
    };
    assert_eq!(
        client.try_set_policy(&dup_policy, &0),
        Err(Ok(PolicyWalletError::InvalidPolicy))
    );
}

#[test]
fn allowlists_reject_duplicate_resource_hashes_deterministically() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    let dup_policy = SpendingPolicy {
        agent,
        max_amount_per_payment: 50,
        max_amount_per_day: 100,
        valid_until_ledger: 100,
        allowed_sellers: vec![&env, seller],
        allowed_assets: vec![&env, asset],
        allowed_resource_hashes: vec![&env, resource_hash.clone(), resource_hash],
        revision: 0,
    };
    assert_eq!(
        client.try_set_policy(&dup_policy, &0),
        Err(Ok(PolicyWalletError::InvalidPolicy))
    );
}

#[test]
fn validation_failures_perform_no_storage_write_and_have_bounded_resource_usage() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    let invalid_policy = SpendingPolicy {
        agent: agent.clone(),
        max_amount_per_payment: 50,
        max_amount_per_day: 100,
        valid_until_ledger: 100,
        allowed_sellers: vec![&env, seller.clone(), seller],
        allowed_assets: vec![&env, asset],
        allowed_resource_hashes: vec![&env, resource_hash],
        revision: 0,
    };

    let result = client.try_set_policy(&invalid_policy, &0);
    assert_eq!(result, Err(Ok(PolicyWalletError::InvalidPolicy)));

    // Policy was NOT stored
    assert_eq!(
        client.try_get_policy(&agent),
        Err(Ok(PolicyWalletError::PolicyNotFound))
    );

    let resources = env.cost_estimate().resources();
    assert!(resources.instructions > 0);
    assert!(resources.mem_bytes > 0);
    assert_eq!(resources.write_entries, 0);
    assert_eq!(resources.write_bytes, 0);
}

#[test]
fn revision_advancement_is_overflow_safe() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, agent, seller, asset, _owner, resource_hash) = initialized_wallet(&env);

    let mut policy = standard_policy(&env, &agent, &seller, &asset, &resource_hash, 100);
    policy.revision = u32::MAX;

    env.as_contract(&client.address, || {
        env.storage()
            .instance()
            .set(&DataKey::Policy(agent.clone()), &policy);
    });

    let res = client.try_set_policy(&policy, &u32::MAX);
    assert_eq!(res, Err(Ok(PolicyWalletError::RevisionOverflow)));
}
