use scrypto::prelude::*;
use crate::types::*;
use crate::constants::*;
use crate::helpers::*;
use crate::curves::*;
use crate::events::*;

/// Opens a long position in the Hyperdrive AMM
/// 
/// # Arguments
/// * `base_tokens` - Bucket of base tokens to use for the position
/// * `pool_state` - Current pool state (mutable)
/// * `long_positions_resource` - Resource address for long position NFTs
/// * `governance_vault` - Vault for governance fees (mutable)
/// * `yield_source_vault` - Vault for yield source tokens (mutable)
/// 
/// # Returns
/// * Long position NFT bucket
pub fn open_long_position(
    base_tokens: Bucket,
    share_reserves: &mut Decimal,
    bond_reserves: &mut Decimal,
    zeta_adjustment: &mut Decimal,
    share_price: Decimal,
    checkpoints: &mut HashMap<u64, Checkpoint>,
    current_checkpoint: &mut u64,
    checkpoint_duration: u64,
    position_duration: u64,
    new_bond_fee: Decimal,
    governance_fee: Decimal,
    long_positions_resource: ResourceAddress,
    yield_source: ResourceAddress,
    governance_vault: &mut Vault,
    yield_source_vault: &mut Vault
) -> Bucket {
    // Validate input
    validate_resource_address(base_tokens.resource_address(), yield_source, "yield source");
    
    // Update checkpoint if needed
    *current_checkpoint = update_checkpoint_if_needed(
        checkpoints, 
        *current_checkpoint, 
        checkpoint_duration, 
        share_price
    );
    
    // Calculate current time and maturity
    let current_time = Runtime::current_epoch().number();
    let maturity_time = *current_checkpoint + position_duration;
    let time_remaining = math::ONE; // Full term for new positions
    
    // Convert base tokens to shares
    let base_amount = base_tokens.amount();
    let share_amount = base_amount / share_price;
    
    // Calculate effective share reserves
    let effective_shares = calculate_effective_share_reserves(*share_reserves, *zeta_adjustment);
    
    // Calculate face value using trading curve
    let face_value = calculate_long_face_value(share_amount, effective_shares, *bond_reserves);
    
    // Calculate fees
    let spot_rate = calculate_spot_rate(effective_shares, *bond_reserves, share_price);
    let (new_bond_fee_amount, _) = calculate_position_fees(
        face_value, 
        time_remaining, 
        spot_rate, 
        new_bond_fee, 
        math::ZERO
    );
    
    let total_fee = new_bond_fee_amount;
    let governance_fee_amount = total_fee * governance_fee;
    let lp_fee = total_fee - governance_fee_amount;
    
    // Adjust face value for fees
    let adjusted_face_value = face_value - lp_fee;
    
    // Update pool state
    *share_reserves += share_amount;
    *bond_reserves -= adjusted_face_value;
    
    // Update checkpoint data
    update_checkpoint_long_opened(checkpoints, *current_checkpoint, adjusted_face_value, maturity_time);
    
    // Create position NFT
    let position_data = LongPosition {
        face_value: adjusted_face_value,
        checkpoint: *current_checkpoint,
        open_time: current_time,
        maturity_time,
    };
    
    let position_nft = NonFungibleResourceManager::from(long_positions_resource)
        .mint_ruid_non_fungible(position_data).into();
    
    // Handle governance fee
    if governance_fee_amount > math::ZERO {
        let governance_fee_bucket = yield_source_vault.take(governance_fee_amount / share_price);
        governance_vault.put(governance_fee_bucket);
    }
    
    // Deposit base tokens
    yield_source_vault.put(base_tokens);
    
    position_nft
}

/// Closes a long position in the Hyperdrive AMM
/// 
/// # Arguments
/// * `position_nft` - Long position NFT to close
/// * `pool_state` - Current pool state (mutable)
/// * `governance_vault` - Vault for governance fees (mutable)
/// * `yield_source_vault` - Vault for yield source tokens (mutable)
/// 
/// # Returns
/// * Base token proceeds bucket
pub fn close_long_position(
    position_nft: Bucket,
    share_reserves: &mut Decimal,
    bond_reserves: &mut Decimal,
    zeta_adjustment: &mut Decimal,
    share_price: Decimal,
    checkpoints: &mut HashMap<u64, Checkpoint>,
    current_checkpoint: &mut u64,
    checkpoint_duration: u64,
    new_bond_fee: Decimal,
    matured_bond_fee: Decimal,
    governance_fee: Decimal,
    long_positions_resource: ResourceAddress,
    governance_vault: &mut Vault,
    yield_source_vault: &mut Vault
) -> Bucket {
    // Validate input
    validate_single_nft(&position_nft, long_positions_resource, "long position");
    
    // Update checkpoint if needed
    *current_checkpoint = update_checkpoint_if_needed(
        checkpoints, 
        *current_checkpoint, 
        checkpoint_duration, 
        share_price
    );
    
    // Get position data
    let position_data: LongPosition = position_nft.as_non_fungible().non_fungible().data();
    
    // Calculate time remaining
    let current_time = Runtime::current_epoch().number();
    let time_remaining = calculate_time_remaining(
        current_time, 
        position_data.open_time, 
        position_data.maturity_time
    );
    
    // Calculate proceeds from closing the position
    let face_value = position_data.face_value;
    let effective_shares = calculate_effective_share_reserves(*share_reserves, *zeta_adjustment);
    
    // Calculate impact on reserves
    let delta_z = position_impact_delta_z(
        face_value,
        effective_shares,
        *bond_reserves,
        time_remaining,
        share_price
    );
    
    // Calculate fees
    let spot_rate = calculate_spot_rate(effective_shares, *bond_reserves, share_price);
    let (new_bond_fee_amount, matured_bond_fee_amount) = calculate_position_fees(
        face_value, 
        time_remaining, 
        spot_rate, 
        new_bond_fee, 
        matured_bond_fee
    );
    
    let total_fee = new_bond_fee_amount + matured_bond_fee_amount;
    let governance_fee_amount = total_fee * governance_fee;
    let lp_fee = total_fee - governance_fee_amount;
    
    // Calculate base proceeds
    let base_proceeds = delta_z * share_price - lp_fee;
    
    // Update pool state
    *share_reserves -= delta_z;
    *bond_reserves += face_value * time_remaining;
    
    // Update zeta adjustment for matured portion
    let matured_impact = maturity_pricing_delta_z(
        face_value * (math::ONE - time_remaining), 
        share_price
    );
    *zeta_adjustment -= matured_impact - (matured_bond_fee_amount * (math::ONE - governance_fee));
    
    // Update checkpoint data
    update_checkpoint_long_closed(checkpoints, position_data.checkpoint, face_value, time_remaining);
    
    // Burn position NFT
    position_nft.burn();
    
    // Handle governance fee
    if governance_fee_amount > math::ZERO {
        let governance_fee_bucket = yield_source_vault.take(governance_fee_amount / share_price);
        governance_vault.put(governance_fee_bucket);
    }
    
    // Return base proceeds
    yield_source_vault.take(base_proceeds)
}

/// Opens a short position in the Hyperdrive AMM
/// 
/// # Arguments
/// * `base_tokens` - Bucket of base tokens for collateral
/// * `face_value` - Desired face value of the short position
/// * `pool_state` - Current pool state (mutable)
/// * `short_positions_resource` - Resource address for short position NFTs
/// * `governance_vault` - Vault for governance fees (mutable)
/// * `yield_source_vault` - Vault for yield source tokens (mutable)
/// 
/// # Returns
/// * Tuple of (short position NFT, change bucket)
pub fn open_short_position(
    mut base_tokens: Bucket,
    face_value: Decimal,
    share_reserves: &mut Decimal,
    bond_reserves: &mut Decimal,
    zeta_adjustment: &mut Decimal,
    share_price: Decimal,
    checkpoints: &mut HashMap<u64, Checkpoint>,
    current_checkpoint: &mut u64,
    checkpoint_duration: u64,
    position_duration: u64,
    new_bond_fee: Decimal,
    governance_fee: Decimal,
    short_positions_resource: ResourceAddress,
    yield_source: ResourceAddress,
    governance_vault: &mut Vault,
    yield_source_vault: &mut Vault
) -> (Bucket, Bucket) {
    // Validate input
    validate_resource_address(base_tokens.resource_address(), yield_source, "yield source");
    assert!(face_value > math::ZERO, "Face value must be positive");
    
    // Update checkpoint if needed
    *current_checkpoint = update_checkpoint_if_needed(
        checkpoints, 
        *current_checkpoint, 
        checkpoint_duration, 
        share_price
    );
    
    // Calculate current time and maturity
    let current_time = Runtime::current_epoch().number();
    let maturity_time = *current_checkpoint + position_duration;
    let time_remaining = math::ONE; // Full term for new positions
    
    // Calculate effective share reserves
    let effective_shares = calculate_effective_share_reserves(*share_reserves, *zeta_adjustment);
    
    // Calculate required deposit
    let deposit_required = calculate_short_deposit(
        face_value,
        effective_shares,
        *bond_reserves,
        share_price,
        time_remaining
    );
    
    // Calculate fees
    let spot_rate = calculate_spot_rate(effective_shares, *bond_reserves, share_price);
    let (new_bond_fee_amount, _) = calculate_position_fees(
        face_value, 
        time_remaining, 
        spot_rate, 
        new_bond_fee, 
        math::ZERO
    );
    
    let total_fee = new_bond_fee_amount;
    let governance_fee_amount = total_fee * governance_fee;
    let lp_fee = total_fee - governance_fee_amount;
    
    // Add fees to required deposit
    let total_deposit_required = deposit_required + lp_fee;
    
    // Ensure sufficient deposit
    assert!(
        base_tokens.amount() >= total_deposit_required,
        "Insufficient deposit for short position"
    );
    
    // Calculate impact on reserves
    let delta_z = position_impact_delta_z(
        face_value,
        effective_shares,
        *bond_reserves,
        time_remaining,
        share_price
    );
    
    // Update pool state
    *share_reserves -= delta_z - (lp_fee / share_price);
    *bond_reserves += face_value;
    
    // Update checkpoint data
    update_checkpoint_short_opened(checkpoints, *current_checkpoint, face_value, maturity_time);
    
    // Create position NFT
    let position_data = ShortPosition {
        face_value,
        checkpoint: *current_checkpoint,
        open_time: current_time,
        maturity_time,
        initial_share_price: share_price,
    };
    
    let position_nft = NonFungibleResourceManager::from(short_positions_resource)
        .mint_ruid_non_fungible(position_data).into();
    
    // Take required deposit and return change
    let change = base_tokens.take(base_tokens.amount() - total_deposit_required);
    
    // Handle governance fee
    if governance_fee_amount > math::ZERO {
        let governance_fee_bucket = yield_source_vault.take(governance_fee_amount / share_price);
        governance_vault.put(governance_fee_bucket);
    }
    
    // Deposit collateral
    yield_source_vault.put(base_tokens);
    
    (position_nft, change)
}

/// Closes a short position in the Hyperdrive AMM
/// 
/// # Arguments
/// * `position_nft` - Short position NFT to close
/// * `pool_state` - Current pool state (mutable)
/// * `governance_vault` - Vault for governance fees (mutable)
/// * `yield_source_vault` - Vault for yield source tokens (mutable)
/// 
/// # Returns
/// * Base token proceeds bucket
pub fn close_short_position(
    position_nft: Bucket,
    share_reserves: &mut Decimal,
    bond_reserves: &mut Decimal,
    zeta_adjustment: &mut Decimal,
    share_price: Decimal,
    checkpoints: &mut HashMap<u64, Checkpoint>,
    current_checkpoint: &mut u64,
    checkpoint_duration: u64,
    new_bond_fee: Decimal,
    matured_bond_fee: Decimal,
    governance_fee: Decimal,
    short_positions_resource: ResourceAddress,
    governance_vault: &mut Vault,
    yield_source_vault: &mut Vault
) -> Bucket {
    // Validate input
    validate_single_nft(&position_nft, short_positions_resource, "short position");
    
    // Update checkpoint if needed
    *current_checkpoint = update_checkpoint_if_needed(
        checkpoints, 
        *current_checkpoint, 
        checkpoint_duration, 
        share_price
    );
    
    // Get position data
    let position_data: ShortPosition = position_nft.as_non_fungible().non_fungible().data();
    
    // Calculate time remaining
    let current_time = Runtime::current_epoch().number();
    let time_remaining = calculate_time_remaining(
        current_time, 
        position_data.open_time, 
        position_data.maturity_time
    );
    
    // Calculate proceeds from closing the position
    let face_value = position_data.face_value;
    let effective_shares = calculate_effective_share_reserves(*share_reserves, *zeta_adjustment);
    
    // Calculate impact on reserves
    let delta_z = position_impact_delta_z(
        face_value,
        effective_shares,
        *bond_reserves,
        time_remaining,
        share_price
    );
    
    // Calculate fees
    let spot_rate = calculate_spot_rate(effective_shares, *bond_reserves, share_price);
    let (new_bond_fee_amount, matured_bond_fee_amount) = calculate_position_fees(
        face_value, 
        time_remaining, 
        spot_rate, 
        new_bond_fee, 
        matured_bond_fee
    );
    
    let total_fee = new_bond_fee_amount + matured_bond_fee_amount;
    let governance_fee_amount = total_fee * governance_fee;
    let lp_fee = total_fee - governance_fee_amount;
    
    // Calculate base proceeds
    // For shorts: proceeds = (face_value * current_share_price / initial_share_price) - cost - fees
    let share_price_ratio = share_price / position_data.initial_share_price;
    let base_proceeds = (face_value * share_price_ratio) - 
                       (delta_z * share_price) - 
                       lp_fee;
    
    // Update pool state
    *share_reserves += delta_z;
    *bond_reserves -= face_value * time_remaining;
    
    // Update zeta adjustment for matured portion
    let matured_impact = maturity_pricing_delta_z(
        face_value * (math::ONE - time_remaining), 
        share_price
    );
    *zeta_adjustment += matured_impact + (matured_bond_fee_amount * (math::ONE - governance_fee) / share_price);
    
    // Update checkpoint data
    update_checkpoint_short_closed(checkpoints, position_data.checkpoint, face_value, time_remaining);
    
    // Burn position NFT
    position_nft.burn();
    
    // Handle governance fee
    if governance_fee_amount > math::ZERO {
        let governance_fee_bucket = yield_source_vault.take(governance_fee_amount / share_price);
        governance_vault.put(governance_fee_bucket);
    }
    
    // Return base proceeds
    yield_source_vault.take(base_proceeds)
}
