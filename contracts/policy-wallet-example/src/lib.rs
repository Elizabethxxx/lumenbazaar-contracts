#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, Vec};

pub const LEDGERS_PER_DAY: u32 = 17_280;
pub const MAX_ALLOWED_SELLERS: u32 = 16;
pub const MAX_ALLOWED_ASSETS: u32 = 16;
pub const MAX_ALLOWED_RESOURCE_HASHES: u32 = 16;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Config,
    Policy(Address),
    DailySpend(Address, u32),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletConfig {
    pub owner: Address,
    pub upto_session_contract: Address,
    pub initialized_at_ledger: u32,
    pub example_only: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpendingPolicy {
    pub agent: Address,
    pub max_amount_per_payment: i128,
    pub max_amount_per_day: i128,
    pub valid_until_ledger: u32,
    pub allowed_sellers: Vec<Address>,
    pub allowed_assets: Vec<Address>,
    pub allowed_resource_hashes: Vec<BytesN<32>>,
    pub revision: u32,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PolicyWalletError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidPolicy = 3,
    PolicyNotFound = 4,
    PolicyExpired = 5,
    InvalidAmount = 6,
    AmountExceedsPaymentCap = 7,
    AmountExceedsDailyCap = 8,
    SellerNotAllowed = 9,
    AssetNotAllowed = 10,
    ResourceHashNotAllowed = 11,
    StaleRevision = 12,
    RevisionOverflow = 13,
}

#[contract]
pub struct PolicyWalletExampleContract;

#[contractimpl]
impl PolicyWalletExampleContract {
    pub fn initialize(
        env: Env,
        owner: Address,
        upto_session_contract: Address,
    ) -> Result<(), PolicyWalletError> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(PolicyWalletError::AlreadyInitialized);
        }

        owner.require_auth();

        let config = WalletConfig {
            owner,
            upto_session_contract,
            initialized_at_ledger: env.ledger().sequence(),
            example_only: true,
        };
        env.storage().instance().set(&DataKey::Config, &config);

        Ok(())
    }

    pub fn get_config(env: Env) -> Result<WalletConfig, PolicyWalletError> {
        read_config(&env)
    }

    pub fn require_owner(env: Env) -> Result<(), PolicyWalletError> {
        require_owner_auth(&env)?;
        Ok(())
    }

    pub fn set_policy(
        env: Env,
        mut policy: SpendingPolicy,
        expected_revision: u32,
    ) -> Result<(), PolicyWalletError> {
        require_owner_auth(&env)?;
        validate_policy(
            &env,
            policy.max_amount_per_payment,
            policy.max_amount_per_day,
            policy.valid_until_ledger,
            &policy.allowed_sellers,
            &policy.allowed_assets,
            &policy.allowed_resource_hashes,
        )?;

        let agent = policy.agent.clone();
        let current_revision = match read_policy(&env, &agent) {
            Ok(existing) => existing.revision,
            Err(PolicyWalletError::PolicyNotFound) => 0,
            Err(e) => return Err(e),
        };

        if current_revision != expected_revision {
            return Err(PolicyWalletError::StaleRevision);
        }

        let next_revision = current_revision
            .checked_add(1)
            .ok_or(PolicyWalletError::RevisionOverflow)?;

        policy.revision = next_revision;
        env.storage()
            .instance()
            .set(&DataKey::Policy(agent), &policy);

        Ok(())
    }

    pub fn create_policy(env: Env, policy: SpendingPolicy) -> Result<(), PolicyWalletError> {
        Self::set_policy(env, policy, 0)
    }

    pub fn get_policy(env: Env, agent: Address) -> Result<SpendingPolicy, PolicyWalletError> {
        read_policy(&env, &agent)
    }

    pub fn get_policy_revision(env: Env, agent: Address) -> Result<u32, PolicyWalletError> {
        Ok(read_policy(&env, &agent)?.revision)
    }

    pub fn check_payment(
        env: Env,
        agent: Address,
        seller: Address,
        asset: Address,
        amount: i128,
        resource_hash: BytesN<32>,
    ) -> Result<(), PolicyWalletError> {
        validate_payment(&env, &agent, &seller, &asset, amount, &resource_hash)?;
        Ok(())
    }

    pub fn authorize_payment(
        env: Env,
        agent: Address,
        seller: Address,
        asset: Address,
        amount: i128,
        resource_hash: BytesN<32>,
    ) -> Result<(), PolicyWalletError> {
        agent.require_auth();
        validate_payment(&env, &agent, &seller, &asset, amount, &resource_hash)?;

        let ledger_day = current_ledger_day(&env);
        let spent = read_daily_spend(&env, &agent, ledger_day);
        let next_spent = spent
            .checked_add(amount)
            .ok_or(PolicyWalletError::AmountExceedsDailyCap)?;
        env.storage()
            .instance()
            .set(&DataKey::DailySpend(agent, ledger_day), &next_spent);

        Ok(())
    }

    pub fn spent_today(env: Env, agent: Address) -> i128 {
        read_daily_spend(&env, &agent, current_ledger_day(&env))
    }

    pub fn example_only(_env: Env) -> bool {
        true
    }
}

fn read_config(env: &Env) -> Result<WalletConfig, PolicyWalletError> {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .ok_or(PolicyWalletError::NotInitialized)
}

fn require_owner_auth(env: &Env) -> Result<(), PolicyWalletError> {
    read_config(env)?.owner.require_auth();
    Ok(())
}

fn read_policy(env: &Env, agent: &Address) -> Result<SpendingPolicy, PolicyWalletError> {
    env.storage()
        .instance()
        .get(&DataKey::Policy(agent.clone()))
        .ok_or(PolicyWalletError::PolicyNotFound)
}

fn has_duplicate_addresses(items: &Vec<Address>) -> bool {
    let len = items.len();
    for i in 0..len {
        let item = items.get(i).unwrap();
        for j in (i + 1)..len {
            if item == items.get(j).unwrap() {
                return true;
            }
        }
    }
    false
}

fn has_duplicate_hashes(items: &Vec<BytesN<32>>) -> bool {
    let len = items.len();
    for i in 0..len {
        let item = items.get(i).unwrap();
        for j in (i + 1)..len {
            if item == items.get(j).unwrap() {
                return true;
            }
        }
    }
    false
}

fn validate_policy(
    env: &Env,
    max_amount_per_payment: i128,
    max_amount_per_day: i128,
    valid_until_ledger: u32,
    allowed_sellers: &Vec<Address>,
    allowed_assets: &Vec<Address>,
    allowed_resource_hashes: &Vec<BytesN<32>>,
) -> Result<(), PolicyWalletError> {
    if max_amount_per_payment <= 0
        || max_amount_per_day <= 0
        || max_amount_per_payment > max_amount_per_day
        || allowed_sellers.is_empty()
        || allowed_sellers.len() > MAX_ALLOWED_SELLERS
        || has_duplicate_addresses(allowed_sellers)
        || allowed_assets.is_empty()
        || allowed_assets.len() > MAX_ALLOWED_ASSETS
        || has_duplicate_addresses(allowed_assets)
        || allowed_resource_hashes.is_empty()
        || allowed_resource_hashes.len() > MAX_ALLOWED_RESOURCE_HASHES
        || has_duplicate_hashes(allowed_resource_hashes)
    {
        return Err(PolicyWalletError::InvalidPolicy);
    }

    if valid_until_ledger <= env.ledger().sequence() {
        return Err(PolicyWalletError::PolicyExpired);
    }

    Ok(())
}

fn validate_payment(
    env: &Env,
    agent: &Address,
    seller: &Address,
    asset: &Address,
    amount: i128,
    resource_hash: &BytesN<32>,
) -> Result<(), PolicyWalletError> {
    let policy = read_policy(env, agent)?;

    if env.ledger().sequence() > policy.valid_until_ledger {
        return Err(PolicyWalletError::PolicyExpired);
    }
    if amount <= 0 {
        return Err(PolicyWalletError::InvalidAmount);
    }
    if amount > policy.max_amount_per_payment {
        return Err(PolicyWalletError::AmountExceedsPaymentCap);
    }
    if !policy.allowed_sellers.contains(seller) {
        return Err(PolicyWalletError::SellerNotAllowed);
    }
    if !policy.allowed_assets.contains(asset) {
        return Err(PolicyWalletError::AssetNotAllowed);
    }
    if !policy.allowed_resource_hashes.contains(resource_hash) {
        return Err(PolicyWalletError::ResourceHashNotAllowed);
    }

    let spent = read_daily_spend(env, agent, current_ledger_day(env));
    let next_spent = spent
        .checked_add(amount)
        .ok_or(PolicyWalletError::AmountExceedsDailyCap)?;
    if next_spent > policy.max_amount_per_day {
        return Err(PolicyWalletError::AmountExceedsDailyCap);
    }

    Ok(())
}

fn read_daily_spend(env: &Env, agent: &Address, ledger_day: u32) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::DailySpend(agent.clone(), ledger_day))
        .unwrap_or(0)
}

fn current_ledger_day(env: &Env) -> u32 {
    env.ledger().sequence() / LEDGERS_PER_DAY
}

#[cfg(test)]
mod test;
