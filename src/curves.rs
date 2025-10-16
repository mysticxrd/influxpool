use scrypto::prelude::*;
use crate::constants::*;

/// Implements the trading invariant function I(Δy, z, y)
/// This calculates the change in share reserves for a given change in bond reserves
/// 
/// # Arguments
/// * `delta_y` - Change in bond reserves (face value of bonds)
/// * `z` - Current share reserves
/// * `y` - Current bond reserves
/// 
/// # Returns
/// * Change in share reserves (Δz)
pub fn trading_invariant_delta_z(delta_y: Decimal, z: Decimal, y: Decimal) -> Decimal {
    if z <= math::ZERO || y <= math::ZERO {
        // If pool is empty, use 1:1 ratio
        return delta_y;
    }
    
    // For constant product curve: (z * y) = (z + Δz) * (y - Δy)
    // Therefore: Δz = ((z * y) / (y - Δy)) - z
    let k = z * y;
    let new_y = y - delta_y;
    
    if new_y <= math::ZERO {
        // Cannot trade more bonds than available
        return math::ZERO;
    }
    
    let new_z = k / new_y;
    new_z - z
}

/// Implements the maturity pricing function M(Δy)
/// This calculates the share cost for bonds at maturity
/// 
/// # Arguments
/// * `delta_y` - Face value of matured bonds
/// * `share_price` - Current share price (c)
/// 
/// # Returns
/// * Share cost for matured bonds
pub fn maturity_pricing_delta_z(delta_y: Decimal, share_price: Decimal) -> Decimal {
    if share_price <= math::ZERO {
        return math::ZERO;
    }
    
    // At maturity, bonds are worth their face value
    // M(Δy) = Δy / c
    delta_y / share_price
}

/// Implements the position impact function H(Δy, z, y, tr)
/// This combines the trading invariant and maturity pricing based on time remaining
/// 
/// # Arguments
/// * `delta_y` - Face value of the position
/// * `z` - Current share reserves
/// * `y` - Current bond reserves
/// * `time_remaining` - Time remaining until maturity (0 to 1)
/// * `share_price` - Current share price
/// 
/// # Returns
/// * Total impact on share reserves
pub fn position_impact_delta_z(
    delta_y: Decimal, 
    z: Decimal, 
    y: Decimal, 
    time_remaining: Decimal,
    share_price: Decimal
) -> Decimal {
    // H(Δy, z, y, tr) = I(Δy*tr, z, y) + M(Δy*(1-tr))
    let new_bonds = delta_y * time_remaining;
    let matured_bonds = delta_y * (math::ONE - time_remaining);
    
    let impact_new = if new_bonds > math::ZERO {
        trading_invariant_delta_z(new_bonds, z, y)
    } else {
        math::ZERO
    };
    
    let impact_matured = if matured_bonds > math::ZERO {
        maturity_pricing_delta_z(matured_bonds, share_price)
    } else {
        math::ZERO
    };
    
    impact_new + impact_matured
}

/// Calculates fees for a position based on time remaining
/// 
/// # Arguments
/// * `delta_y` - Face value of the position
/// * `time_remaining` - Time remaining until maturity (0 to 1)
/// * `spot_rate` - Current spot rate
/// * `new_bond_fee` - Fee percentage for newly minted bonds
/// * `matured_bond_fee` - Fee percentage for matured bonds
/// 
/// # Returns
/// * Tuple of (new_bond_fee_amount, matured_bond_fee_amount)
pub fn calculate_position_fees(
    delta_y: Decimal,
    time_remaining: Decimal,
    spot_rate: Decimal,
    new_bond_fee: Decimal,
    matured_bond_fee: Decimal
) -> (Decimal, Decimal) {
    // Calculate fees for new and matured bonds
    let new_bond_fee_amount = new_bond_fee * 
        (math::ONE - spot_rate) * 
        delta_y * 
        time_remaining;
        
    let matured_bond_fee_amount = matured_bond_fee * 
        delta_y * 
        (math::ONE - time_remaining);
        
    (new_bond_fee_amount, matured_bond_fee_amount)
}

/// Calculates the LP present value for liquidity operations
/// This is a simplified calculation - actual implementation would be more complex
/// 
/// # Arguments
/// * `share_reserves` - Current share reserves
/// * `bond_reserves` - Current bond reserves
/// * `share_price` - Current share price
/// * `active_lp_shares` - Total active LP shares
/// 
/// # Returns
/// * LP present value per share
pub fn calculate_lp_present_value(
    share_reserves: Decimal,
    bond_reserves: Decimal,
    share_price: Decimal,
    active_lp_shares: Decimal
) -> Decimal {
    if active_lp_shares <= math::ZERO {
        return math::ONE;
    }
    
    // Calculate total pool value
    let share_value = share_reserves * share_price;
    let bond_value = bond_reserves; // Bonds are valued at face value
    let total_value = share_value + bond_value;
    
    // Return value per LP share
    total_value / active_lp_shares
}

/// Calculates the face value for a long position based on share input
/// This uses the trading invariant to determine how many bonds can be purchased
/// 
/// # Arguments
/// * `share_amount` - Amount of shares being used to open the position
/// * `effective_share_reserves` - Current effective share reserves
/// * `bond_reserves` - Current bond reserves
/// 
/// # Returns
/// * Face value of bonds that can be purchased
pub fn calculate_long_face_value(
    share_amount: Decimal,
    effective_share_reserves: Decimal,
    bond_reserves: Decimal
) -> Decimal {
    if effective_share_reserves <= math::ZERO || bond_reserves <= math::ZERO {
        // If pool is empty, use 1:1 ratio
        return share_amount;
    }
    
    // For constant product: (z * y) = (z + Δz) * (y - Δy)
    // Therefore: Δy = y - ((z * y) / (z + Δz))
    let k = effective_share_reserves * bond_reserves;
    let new_z = effective_share_reserves + share_amount;
    
    bond_reserves - (k / new_z)
}

/// Calculates the required deposit for a short position
/// 
/// # Arguments
/// * `face_value` - Desired face value of the short position
/// * `effective_share_reserves` - Current effective share reserves
/// * `bond_reserves` - Current bond reserves
/// * `share_price` - Current share price
/// * `time_remaining` - Time remaining until maturity
/// 
/// # Returns
/// * Required deposit amount in base tokens
pub fn calculate_short_deposit(
    face_value: Decimal,
    effective_share_reserves: Decimal,
    bond_reserves: Decimal,
    share_price: Decimal,
    time_remaining: Decimal
) -> Decimal {
    // Calculate impact on reserves
    let delta_z = position_impact_delta_z(
        face_value,
        effective_share_reserves,
        bond_reserves,
        time_remaining,
        share_price
    );
    
    // Calculate required deposit
    // For shorts, deposit = (face_value * share_price) - (delta_z * share_price)
    let collateral_required = face_value * share_price;
    let proceeds_received = delta_z * share_price;
    
    collateral_required - proceeds_received
}

/// Validates trading parameters before executing trades
/// 
/// # Arguments
/// * `effective_share_reserves` - Current effective share reserves
/// * `bond_reserves` - Current bond reserves
/// * `delta_y` - Requested change in bond reserves
pub fn validate_trading_parameters(
    effective_share_reserves: Decimal,
    bond_reserves: Decimal,
    delta_y: Decimal
) {
    assert!(effective_share_reserves > math::ZERO, "Effective share reserves must be positive");
    assert!(bond_reserves > math::ZERO, "Bond reserves must be positive");
    assert!(delta_y > math::ZERO, "Delta Y must be positive");
    assert!(delta_y < bond_reserves, "Cannot trade more bonds than available");
}
