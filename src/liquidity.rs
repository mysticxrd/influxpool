use scrypto::prelude::*;
use crate::types::*;
use crate::constants::*;
use crate::helpers::*;
use crate::curves::*;
use crate::events::*;

/// Adds liquidity to the Hyperdrive AMM pool
/// 
/// # Arguments
/// * `base_tokens` - Bucket of base tokens to add as liquidity
/// * `pool_state` - Current pool state (mutable)
/// * `active_lp_shares_resource` - Resource address for LP tokens
/// * `yield_source_vault` - Vault for yield source tokens (mutable)
/// 
/// # Returns
/// * LP token bucket
pub fn add_liquidity(
    base_tokens: Bucket,
    share_reserves: &mut Decimal,
    bond_reserves: &mut Decimal,
    share_price: Decimal,
    active_lp_shares: Decimal,
    active_lp_shares_resource: ResourceAddress,
    yield_source: ResourceAddress,
    yield_source_vault: &mut Vault
) -> Bucket {
    // Validate input
    validate_resource_address(base_tokens.resource_address(), yield_source, "yield source");
    
    let base_amount = base_tokens.amount();
    validate_liquidity_amount(base_amount, validation::MIN_LIQUIDITY);
    
    // Convert base tokens to shares
    let share_amount = base_amount / share_price;
    
    // Calculate LP tokens to mint
    let lp_tokens_to_mint = if active_lp_shares <= math::ZERO {
        // First liquidity provision - mint 1:1 with shares
        share_amount
    } else {
        // Calculate based on current pool value
        let lp_present_value = calculate_lp_present_value(
            *share_reserves,
            *bond_reserves,
            share_price,
            active_lp_shares
        );
        
        // LP tokens = (share_amount * share_price) / lp_present_value
        (share_amount * share_price) / lp_present_value
    };
    
    // Update pool state
    *share_reserves += share_amount;
    
    // Deposit base tokens
    yield_source_vault.put(base_tokens);
    
    // Mint and return LP tokens
    FungibleResourceManager::from(active_lp_shares_resource).mint(lp_tokens_to_mint).into()

}

/// Removes liquidity from the Hyperdrive AMM pool
/// 
/// # Arguments
/// * `lp_tokens` - LP tokens to burn for liquidity removal
/// * `pool_state` - Current pool state (mutable)
/// * `withdrawal_shares_resource` - Resource address for withdrawal tokens
/// * `yield_source_vault` - Vault for yield source tokens (mutable)
/// 
/// # Returns
/// * Tuple of (base tokens, withdrawal shares)
pub fn remove_liquidity(
    lp_tokens: Bucket,
    share_reserves: &mut Decimal,
    bond_reserves: &mut Decimal,
    share_price: Decimal,
    active_lp_shares: Decimal,
    min_share_reserves: Decimal,
    checkpoints: &HashMap<u64, Checkpoint>,
    current_checkpoint: u64,
    checkpoint_duration: u64,
    position_duration: u64,
    active_lp_shares_resource: ResourceAddress,
    withdrawal_shares_resource: ResourceAddress,
    yield_source_vault: &mut Vault
) -> (Bucket, Bucket) {
    // Validate input
    validate_resource_address(lp_tokens.resource_address(), active_lp_shares_resource, "LP token");
    
    let lp_amount = lp_tokens.amount();
    assert!(lp_amount > math::ZERO, "LP amount must be positive");
    assert!(lp_amount <= active_lp_shares, "Insufficient LP tokens");
    
    // Calculate LP present value
    let lp_present_value = calculate_lp_present_value(
        *share_reserves,
        *bond_reserves,
        share_price,
        active_lp_shares
    );
    
    // Calculate total value to withdraw
    let total_value_to_withdraw = lp_amount * lp_present_value;
    
    // Calculate solvency requirement
    let solvency_requirement = calculate_solvency_requirement(
        checkpoints,
        current_checkpoint,
        checkpoint_duration,
        position_duration
    );
    
    // Calculate available liquidity for immediate withdrawal
    let available_share_value = (*share_reserves - min_share_reserves - (solvency_requirement / share_price))
        .max(math::ZERO) * share_price;
    
    // Determine immediate withdrawal amount
    let immediate_withdrawal = total_value_to_withdraw.min(available_share_value);
    let withdrawal_shares_amount = total_value_to_withdraw - immediate_withdrawal;
    
    // Update pool state
    if immediate_withdrawal > math::ZERO {
        *share_reserves -= immediate_withdrawal / share_price;
    }
    
    // Burn LP tokens
    lp_tokens.burn();
    
    // Create buckets to return
    let base_tokens = if immediate_withdrawal > math::ZERO {
        yield_source_vault.take(immediate_withdrawal)
    } else {
        Bucket::new(yield_source_vault.resource_address())
    };
    
    let withdrawal_shares = if withdrawal_shares_amount > math::ZERO {
        FungibleResourceManager::from(withdrawal_shares_resource)
            .mint(withdrawal_shares_amount).into()
    } else {
        Bucket::new(withdrawal_shares_resource)
    };
    
    (base_tokens, withdrawal_shares)
}

/// Distributes excess idle liquidity to withdrawal shares
/// 
/// # Arguments
/// * `pool_state` - Current pool state (mutable)
/// * `withdrawal_shares` - Current withdrawal shares amount
/// * `ready_withdrawal_shares_resource` - Resource address for ready withdrawal tokens
/// * `ready_withdrawal_vault` - Vault for ready withdrawal tokens (mutable)
/// 
/// # Returns
/// * Tuple of (idle_liquidity_distributed, ready_withdrawal_shares_minted)
pub fn distribute_excess_idle_liquidity(
    share_reserves: &mut Decimal,
    bond_reserves: &mut Decimal,
    zeta_adjustment: &mut Decimal,
    share_price: Decimal,
    withdrawal_shares: Decimal,
    active_lp_shares: Decimal,
    min_share_reserves: Decimal,
    checkpoints: &HashMap<u64, Checkpoint>,
    current_checkpoint: u64,
    checkpoint_duration: u64,
    position_duration: u64,
    ready_withdrawal_shares_resource: ResourceAddress,
    ready_withdrawal_vault: &mut Vault
) -> (Decimal, Decimal) {
    // Calculate idle liquidity
    let idle_liquidity = calculate_idle_liquidity(
        *share_reserves,
        share_price,
        min_share_reserves,
        checkpoints,
        current_checkpoint,
        checkpoint_duration,
        position_duration
    );
    
    if withdrawal_shares <= math::ZERO || idle_liquidity <= math::ZERO {
        return (math::ZERO, math::ZERO);
    }
    
    // Calculate LP present value
    let lp_present_value = calculate_lp_present_value(
        *share_reserves,
        *bond_reserves,
        share_price,
        active_lp_shares
    );
    
    // Calculate maximum withdrawal shares that can be redeemed
    let max_withdrawal_shares = (idle_liquidity / share_price) * withdrawal_shares / lp_present_value;
    let ready_withdrawal_shares = max_withdrawal_shares.min(withdrawal_shares);
    
    if ready_withdrawal_shares <= math::ZERO {
        return (math::ZERO, math::ZERO);
    }
    
    // Update pool state
    *share_reserves -= idle_liquidity / share_price;
    
    // Update zeta adjustment and bond reserves to maintain spot price
    let share_ratio = *share_reserves / (*share_reserves + (idle_liquidity / share_price));
    *zeta_adjustment = *zeta_adjustment * share_ratio;
    *bond_reserves = *bond_reserves * share_ratio;
    
    // Mint ready withdrawal shares
    let ready_shares = FungibleResourceManager::from(ready_withdrawal_shares_resource)
                .mint(ready_withdrawal_shares).into();
    
    ready_withdrawal_vault.put(ready_shares);
    
    (idle_liquidity, ready_withdrawal_shares)
}

/// Redeems withdrawal shares for base tokens
/// 
/// # Arguments
/// * `withdrawal_shares` - Withdrawal shares to redeem
/// * `ready_withdrawal_shares_amount` - Amount of ready withdrawal shares available
/// * `share_price` - Current share price
/// * `withdrawal_shares_resource` - Resource address for withdrawal tokens
/// * `ready_withdrawal_vault` - Vault for ready withdrawal tokens (mutable)
/// * `yield_source_vault` - Vault for yield source tokens (mutable)
/// 
/// # Returns
/// * Base token bucket
pub fn redeem_withdrawal_shares(
    withdrawal_shares: Bucket,
    ready_withdrawal_shares_amount: Decimal,
    share_price: Decimal,
    withdrawal_shares_resource: ResourceAddress,
    ready_withdrawal_vault: &mut Vault,
    yield_source_vault: &mut Vault
) -> Bucket {
    // Validate input
    validate_resource_address(
        withdrawal_shares.resource_address(), 
        withdrawal_shares_resource, 
        "withdrawal shares"
    );
    
    assert!(
        ready_withdrawal_shares_amount > math::ZERO,
        "No withdrawal shares ready for redemption"
    );
    
    // Calculate redemption amount
    let withdrawal_amount = withdrawal_shares.amount();
    let max_redemption = withdrawal_amount.min(ready_withdrawal_shares_amount);
    
    // Calculate base tokens to return
    let base_tokens_per_share = share_price;
    let base_tokens_amount = max_redemption * base_tokens_per_share;
    
    // Burn withdrawal shares
    withdrawal_shares.burn();
    
    // Burn ready withdrawal shares
    let ready_shares = ready_withdrawal_vault.take(max_redemption);
    ready_shares.burn();
    
    // Return base tokens
    yield_source_vault.take(base_tokens_amount)
}

/// Calculates the idle liquidity available in the pool
/// 
/// # Arguments
/// * `share_reserves` - Current share reserves
/// * `share_price` - Current share price
/// * `min_share_reserves` - Minimum required share reserves
/// * `checkpoints` - Reference to checkpoints HashMap
/// * `current_checkpoint` - Current checkpoint ID
/// * `checkpoint_duration` - Duration of each checkpoint
/// * `position_duration` - Duration of positions
/// 
/// # Returns
/// * Idle liquidity amount in base tokens
pub fn calculate_idle_liquidity(
    share_reserves: Decimal,
    share_price: Decimal,
    min_share_reserves: Decimal,
    checkpoints: &HashMap<u64, Checkpoint>,
    current_checkpoint: u64,
    checkpoint_duration: u64,
    position_duration: u64
) -> Decimal {
    // Calculate solvency requirement
    let solvency_requirement = calculate_solvency_requirement(
        checkpoints,
        current_checkpoint,
        checkpoint_duration,
        position_duration
    );
    
    // Calculate idle liquidity
    let idle_liquidity_shares = (share_reserves - (solvency_requirement / share_price))
        .max(math::ZERO)
        .min(share_reserves - min_share_reserves);
        
    idle_liquidity_shares * share_price
}

/// Updates share price from the yield source
/// This would be called periodically or when interacting with the yield source
/// 
/// # Arguments
/// * `new_share_price` - New share price from yield source
/// * `share_price` - Current share price (mutable)
pub fn update_share_price_from_yield_source(
    new_share_price: Decimal,
    share_price: &mut Decimal
) {
    assert!(new_share_price > math::ZERO, "Share price must be positive");
    *share_price = new_share_price;
}

/// Withdraws governance fees (admin only)
/// 
/// # Arguments
/// * `admin_badge` - Admin badge for authorization
/// * `governance_vault` - Vault containing governance fees (mutable)
/// 
/// # Returns
/// * All governance fees as a bucket
pub fn withdraw_governance_fees(
    admin_badge: ResourceAddress,
    auth: Proof,
    governance_vault: &mut Vault
) -> Bucket {
    // Validate admin authorization
    auth.check(admin_badge);
    
    // Take all governance fees
    governance_vault.take_all()
}
