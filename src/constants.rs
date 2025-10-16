use scrypto::prelude::*;

/// Default minimum share reserves to maintain pool solvency
#[allow(dead_code)]
pub const DEFAULT_MIN_SHARE_RESERVES: Decimal = dec!("1000");

/// Maximum fee percentage (100%)
#[allow(dead_code)]
pub const MAX_FEE_PERCENTAGE: Decimal = dec!("1.0");

/// Default new bond fee percentage
#[allow(dead_code)]
pub const DEFAULT_NEW_BOND_FEE: Decimal = dec!("0.01"); // 1%

/// Default matured bond fee percentage  
#[allow(dead_code)]
pub const DEFAULT_MATURED_BOND_FEE: Decimal = dec!("0.005"); // 0.5%

/// Default governance fee percentage
#[allow(dead_code)]
pub const DEFAULT_GOVERNANCE_FEE: Decimal = dec!("0.1"); // 10% of fees

/// Default zombie governance fee percentage
#[allow(dead_code)]
pub const DEFAULT_ZOMBIE_GOVERNANCE_FEE: Decimal = dec!("0.1"); // 10% of zombie interest

/// Default checkpoint duration (1 week in seconds)
pub const DEFAULT_CHECKPOINT_DURATION: u64 = 604800; // 7 * 24 * 60 * 60

/// Default position duration (1 year in seconds)
#[allow(dead_code)]
pub const DEFAULT_POSITION_DURATION: u64 = 31536000; // 365 * 24 * 60 * 60

/// Minimum position duration (must be at least one checkpoint)
pub const MIN_POSITION_DURATION: u64 = DEFAULT_CHECKPOINT_DURATION;

/// Maximum position duration (10 years in seconds)
pub const MAX_POSITION_DURATION: u64 = 315360000; // 10 * 365 * 24 * 60 * 60;

/// Mathematical constants
pub mod math {
    use scrypto::prelude::*;
    
    /// Zero value for calculations
    pub const ZERO: Decimal = dec!("0");
    /// One value for calculations
    pub const ONE: Decimal = dec!("1");
    /// Small epsilon for floating point comparisons
    #[allow(dead_code)]
    pub const EPSILON: Decimal = dec!("0.000001");
}

/// Validation constants
pub mod validation {
    use scrypto::prelude::*;
    
    /// Minimum liquidity amount
    pub const MIN_LIQUIDITY: Decimal = dec!("1");
    /// Maximum reasonable fee percentage
    #[allow(dead_code)]
    pub const MAX_REASONABLE_FEE: Decimal = dec!("0.1"); // 10%
}
