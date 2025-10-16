use scrypto::prelude::*;
use crate::types::*;
use crate::constants::*;
use crate::helpers::*;

/// Updates the current checkpoint if needed based on current time
/// 
/// # Arguments
/// * `checkpoints` - Mutable reference to the checkpoints HashMap
/// * `current_checkpoint` - Current checkpoint ID
/// * `checkpoint_duration` - Duration of each checkpoint
/// * `share_price` - Current share price
/// 
/// # Returns
/// * New checkpoint ID if updated, otherwise current checkpoint ID
pub fn update_checkpoint_if_needed(
    checkpoints: &mut HashMap<u64, Checkpoint>,
    current_checkpoint: u64,
    checkpoint_duration: u64,
    share_price: Decimal
) -> u64 {
    let current_time = Runtime::current_epoch().number();
    let calculated_checkpoint = calculate_current_checkpoint(current_time, checkpoint_duration);
    
    if should_update_checkpoint(current_checkpoint, calculated_checkpoint) {
        // Create new checkpoint
        let new_checkpoint = Checkpoint {
            start_time: calculated_checkpoint,
            share_price,
            long_positions: math::ZERO,
            short_positions: math::ZERO,
            avg_long_maturity: math::ZERO,
            avg_short_maturity: math::ZERO,
            is_minted: true,
        };
        
        checkpoints.insert(calculated_checkpoint, new_checkpoint);
        calculated_checkpoint
    } else {
        current_checkpoint
    }
}

/// Updates checkpoint data when a long position is opened
/// 
/// # Arguments
/// * `checkpoints` - Mutable reference to the checkpoints HashMap
/// * `checkpoint_id` - Checkpoint ID to update
/// * `face_value` - Face value of the long position
/// * `maturity_time` - Maturity time of the position
pub fn update_checkpoint_long_opened(
    checkpoints: &mut HashMap<u64, Checkpoint>,
    checkpoint_id: u64,
    face_value: Decimal,
    maturity_time: u64
) {
    if let Some(checkpoint) = checkpoints.get_mut(&checkpoint_id) {
        let old_total = checkpoint.long_positions;
        let new_total = old_total + face_value;
        
        // Update weighted average maturity
        if new_total > math::ZERO {
            let old_weighted_maturity = checkpoint.avg_long_maturity * old_total;
            let new_weighted_maturity = Decimal::from(maturity_time) * face_value;
            checkpoint.avg_long_maturity = (old_weighted_maturity + new_weighted_maturity) / new_total;
        }
        
        checkpoint.long_positions = new_total;
    }
}

/// Updates checkpoint data when a long position is closed
/// 
/// # Arguments
/// * `checkpoints` - Mutable reference to the checkpoints HashMap
/// * `checkpoint_id` - Checkpoint ID to update
/// * `face_value` - Face value of the long position being closed
/// * `time_remaining` - Time remaining for the position
pub fn update_checkpoint_long_closed(
    checkpoints: &mut HashMap<u64, Checkpoint>,
    checkpoint_id: u64,
    face_value: Decimal,
    time_remaining: Decimal
) {
    if let Some(checkpoint) = checkpoints.get_mut(&checkpoint_id) {
        checkpoint.long_positions -= face_value;
        
        // Update average maturity (simplified)
        if checkpoint.long_positions > math::ZERO {
            // This is a simplification - actual implementation would recalculate
            // the weighted average maturity properly
            checkpoint.avg_long_maturity = time_remaining;
        } else {
            checkpoint.avg_long_maturity = math::ZERO;
        }
    }
}

/// Updates checkpoint data when a short position is opened
/// 
/// # Arguments
/// * `checkpoints` - Mutable reference to the checkpoints HashMap
/// * `checkpoint_id` - Checkpoint ID to update
/// * `face_value` - Face value of the short position
/// * `maturity_time` - Maturity time of the position
pub fn update_checkpoint_short_opened(
    checkpoints: &mut HashMap<u64, Checkpoint>,
    checkpoint_id: u64,
    face_value: Decimal,
    maturity_time: u64
) {
    if let Some(checkpoint) = checkpoints.get_mut(&checkpoint_id) {
        let old_total = checkpoint.short_positions;
        let new_total = old_total + face_value;
        
        // Update weighted average maturity
        if new_total > math::ZERO {
            let old_weighted_maturity = checkpoint.avg_short_maturity * old_total;
            let new_weighted_maturity = Decimal::from(maturity_time) * face_value;
            checkpoint.avg_short_maturity = (old_weighted_maturity + new_weighted_maturity) / new_total;
        }
        
        checkpoint.short_positions = new_total;
    }
}

/// Updates checkpoint data when a short position is closed
/// 
/// # Arguments
/// * `checkpoints` - Mutable reference to the checkpoints HashMap
/// * `checkpoint_id` - Checkpoint ID to update
/// * `face_value` - Face value of the short position being closed
/// * `time_remaining` - Time remaining for the position
pub fn update_checkpoint_short_closed(
    checkpoints: &mut HashMap<u64, Checkpoint>,
    checkpoint_id: u64,
    face_value: Decimal,
    time_remaining: Decimal
) {
    if let Some(checkpoint) = checkpoints.get_mut(&checkpoint_id) {
        checkpoint.short_positions -= face_value;
        
        // Update average maturity (simplified)
        if checkpoint.short_positions > math::ZERO {
            // This is a simplification - actual implementation would recalculate
            // the weighted average maturity properly
            checkpoint.avg_short_maturity = time_remaining;
        } else {
            checkpoint.avg_short_maturity = math::ZERO;
        }
    }
}

/// Collects zombie interest and updates pool state
/// 
/// # Arguments
/// * `zombie_share_reserves` - Current zombie share reserves
/// * `zombie_base_reserves` - Current zombie base reserves
/// * `share_price` - Current share price
/// * `zombie_governance_fee` - Zombie governance fee percentage
/// 
/// # Returns
/// * Tuple of (total_zombie_interest, governance_portion, lp_portion, new_zombie_share_reserves)
#[allow(dead_code)]
pub fn collect_zombie_interest(
    zombie_share_reserves: Decimal,
    zombie_base_reserves: Decimal,
    share_price: Decimal,
    zombie_governance_fee: Decimal
) -> (Decimal, Decimal, Decimal, Decimal) {
    // Calculate zombie interest
    let zombie_interest = (share_price * zombie_share_reserves) - zombie_base_reserves;
    
    if zombie_interest <= math::ZERO {
        return (math::ZERO, math::ZERO, math::ZERO, zombie_share_reserves);
    }
    
    // Calculate governance and LP portions
    let governance_portion = zombie_interest * zombie_governance_fee;
    let lp_portion = zombie_interest - governance_portion;
    
    // Update zombie share reserves
    let new_zombie_share_reserves = zombie_base_reserves / share_price;
    
    (zombie_interest, governance_portion, lp_portion, new_zombie_share_reserves)
}

/// Calculates solvency requirement for active checkpoints
/// 
/// # Arguments
/// * `checkpoints` - Reference to the checkpoints HashMap
/// * `current_checkpoint` - Current checkpoint ID
/// * `checkpoint_duration` - Duration of each checkpoint
/// * `position_duration` - Duration of positions
/// 
/// # Returns
/// * Total solvency requirement
pub fn calculate_solvency_requirement(
    checkpoints: &HashMap<u64, Checkpoint>,
    current_checkpoint: u64,
    checkpoint_duration: u64,
    position_duration: u64
) -> Decimal {
    let mut solvency_requirement = math::ZERO;
    
    // Calculate checkpoints per term
    let checkpoints_per_term = position_duration / checkpoint_duration;
    
    // Calculate solvency requirement for each active checkpoint
    for i in 0..checkpoints_per_term {
        let checkpoint_id = current_checkpoint.saturating_sub(i * checkpoint_duration);
        
        if let Some(checkpoint) = checkpoints.get(&checkpoint_id) {
            // Calculate checkpoint solvency requirement
            let checkpoint_requirement = (checkpoint.long_positions - checkpoint.short_positions)
                .max(math::ZERO);
                
            solvency_requirement += checkpoint_requirement;
        }
    }
    
    solvency_requirement
}

/// Initializes the first checkpoint for a new pool
/// 
/// # Arguments
/// * `checkpoints` - Mutable reference to the checkpoints HashMap
/// * `checkpoint_duration` - Duration of each checkpoint
/// * `initial_share_price` - Initial share price
/// 
/// # Returns
/// * Initial checkpoint ID
pub fn initialize_first_checkpoint(
    checkpoints: &mut HashMap<u64, Checkpoint>,
    checkpoint_duration: u64,
    initial_share_price: Decimal
) -> u64 {
    let current_time = Runtime::current_epoch().number();
    let current_checkpoint = calculate_current_checkpoint(current_time, checkpoint_duration);
    
    let initial_checkpoint = Checkpoint {
        start_time: current_checkpoint,
        share_price: initial_share_price,
        long_positions: math::ZERO,
        short_positions: math::ZERO,
        avg_long_maturity: math::ZERO,
        avg_short_maturity: math::ZERO,
        is_minted: true,
    };
    
    checkpoints.insert(current_checkpoint, initial_checkpoint);
    current_checkpoint
}

/// Gets the current pool state for external queries
/// 
/// # Arguments
/// * `share_reserves` - Current share reserves
/// * `bond_reserves` - Current bond reserves
/// * `zeta_adjustment` - Current zeta adjustment
/// * `share_price` - Current share price
/// * `active_lp_shares` - Total active LP shares
/// * `withdrawal_shares` - Total withdrawal shares
/// * `ready_withdrawal_shares` - Total ready withdrawal shares
/// * `zombie_share_reserves` - Zombie share reserves
/// * `zombie_base_reserves` - Zombie base reserves
/// * `current_checkpoint` - Current checkpoint ID
/// 
/// # Returns
/// * PoolState struct with current pool information
pub fn get_pool_state(
    share_reserves: Decimal,
    bond_reserves: Decimal,
    zeta_adjustment: Decimal,
    share_price: Decimal,
    active_lp_shares: Decimal,
    withdrawal_shares: Decimal,
    ready_withdrawal_shares: Decimal,
    zombie_share_reserves: Decimal,
    zombie_base_reserves: Decimal,
    current_checkpoint: u64
) -> PoolState {
    let effective_share_reserves = calculate_effective_share_reserves(share_reserves, zeta_adjustment);
    let spot_rate = calculate_spot_rate(effective_share_reserves, bond_reserves, share_price);
    
    PoolState {
        share_reserves,
        bond_reserves,
        zeta_adjustment,
        effective_share_reserves,
        share_price,
        spot_rate,
        active_lp_shares_address: active_lp_shares,
        withdrawal_shares_address: withdrawal_shares,
        ready_withdrawal_shares_address: ready_withdrawal_shares,
        zombie_share_reserves,
        zombie_base_reserves,
        current_checkpoint,
    }
}
