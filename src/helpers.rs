use scrypto::prelude::*;
//use crate::types::*;
use crate::constants::*;

/// Calculates the effective share reserves (ze = z - ζ)
/// 
/// # Arguments
/// * `share_reserves` - Total share reserves (z)
/// * `zeta_adjustment` - Zeta adjustment (ζ)
/// 
/// # Returns
/// * Effective share reserves
pub fn calculate_effective_share_reserves(share_reserves: Decimal, zeta_adjustment: Decimal) -> Decimal {
    share_reserves - zeta_adjustment
}

/// Calculates the current spot rate based on pool state
/// 
/// # Arguments
/// * `effective_share_reserves` - Effective share reserves (ze)
/// * `bond_reserves` - Bond reserves (y)
/// * `share_price` - Current share price (c)
/// 
/// # Returns
/// * Current spot rate
pub fn calculate_spot_rate(
    effective_share_reserves: Decimal, 
    bond_reserves: Decimal, 
    share_price: Decimal
) -> Decimal {
    if effective_share_reserves <= math::ZERO || bond_reserves <= math::ZERO {
        return math::ZERO;
    }
    
    // Spot rate calculation based on the Hyperdrive formula
    // r = (ze * c) / (ze * c + y) - 1
    let numerator = effective_share_reserves * share_price;
    let denominator = numerator + bond_reserves;
    
    if denominator <= math::ZERO {
        return math::ZERO;
    }
    
    (numerator / denominator) - math::ONE
}

/// Validates fee parameters
/// 
/// # Arguments
/// * `fee` - Fee percentage to validate
/// * `fee_name` - Name of the fee for error messages
pub fn validate_fee(fee: Decimal, fee_name: &str) {
    assert!(
        fee >= math::ZERO && fee <= MAX_FEE_PERCENTAGE,
        "{} must be between 0 and 1", 
        fee_name
    );
}

/// Validates duration parameters
/// 
/// # Arguments
/// * `checkpoint_duration` - Checkpoint duration in seconds
/// * `position_duration` - Position duration in seconds
pub fn validate_durations(checkpoint_duration: u64, position_duration: u64) {
    assert!(checkpoint_duration > 0, "Checkpoint duration must be positive");
    assert!(position_duration > 0, "Position duration must be positive");
    assert!(
        position_duration >= MIN_POSITION_DURATION,
        "Position duration must be at least {} seconds",
        MIN_POSITION_DURATION
    );
    assert!(
        position_duration <= MAX_POSITION_DURATION,
        "Position duration must be at most {} seconds",
        MAX_POSITION_DURATION
    );
    assert!(
        position_duration % checkpoint_duration == 0,
        "Position duration must be a multiple of checkpoint duration"
    );
}

/// Validates liquidity amount
/// 
/// # Arguments
/// * `amount` - Liquidity amount to validate
/// * `min_amount` - Minimum required amount
pub fn validate_liquidity_amount(amount: Decimal, min_amount: Decimal) {
    assert!(
        amount >= min_amount,
        "Liquidity amount must be at least {}",
        min_amount
    );
}

/// Validates resource address matches expected
/// 
/// # Arguments
/// * `actual` - Actual resource address
/// * `expected` - Expected resource address
/// * `resource_name` - Name of the resource for error messages
pub fn validate_resource_address(
    actual: ResourceAddress, 
    expected: ResourceAddress, 
    resource_name: &str
) {
    assert!(
        actual == expected,
        "Invalid {} resource address",
        resource_name
    );
}

/// Calculates time remaining for a position
/// 
/// # Arguments
/// * `current_time` - Current epoch time
/// * `open_time` - Time when position was opened
/// * `maturity_time` - Time when position matures
/// 
/// # Returns
/// * Time remaining as a decimal between 0 and 1
pub fn calculate_time_remaining(
    current_time: u64,
    open_time: u64,
    maturity_time: u64
) -> Decimal {
    if current_time >= maturity_time {
        return math::ZERO; // Fully matured
    }
    
    if open_time >= maturity_time {
        return math::ZERO; // Invalid position
    }
    
    let time_passed = current_time.saturating_sub(open_time);
    let total_duration = maturity_time.saturating_sub(open_time);
    
    if total_duration == 0 {
        return math::ZERO;
    }
    
    let remaining_duration = total_duration.saturating_sub(time_passed);
    Decimal::from(remaining_duration) / Decimal::from(total_duration)
}

/// Calculates the current checkpoint ID based on time
/// 
/// # Arguments
/// * `current_time` - Current epoch time
/// * `checkpoint_duration` - Duration of each checkpoint
/// 
/// # Returns
/// * Current checkpoint ID
pub fn calculate_current_checkpoint(current_time: u64, checkpoint_duration: u64) -> u64 {
    current_time - (current_time % checkpoint_duration)
}

/// Checks if a checkpoint should be updated
/// 
/// # Arguments
/// * `current_checkpoint` - Current checkpoint ID
/// * `calculated_checkpoint` - Calculated checkpoint ID based on current time
/// 
/// # Returns
/// * True if checkpoint should be updated
pub fn should_update_checkpoint(current_checkpoint: u64, calculated_checkpoint: u64) -> bool {
    calculated_checkpoint > current_checkpoint
}

/// Validates that a bucket contains exactly one NFT
/// 
/// # Arguments
/// * `bucket` - Bucket to validate
/// * `expected_resource` - Expected NFT resource address
/// * `nft_name` - Name of the NFT for error messages
pub fn validate_single_nft(
    bucket: &Bucket, 
    expected_resource: ResourceAddress, 
    nft_name: &str
) {
    validate_resource_address(bucket.resource_address(), expected_resource, nft_name);
    assert!(bucket.amount() == dec!("1"), "Can only process one {} at a time", nft_name);
}
