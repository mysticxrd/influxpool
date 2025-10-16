use scrypto::prelude::*;

/// Checkpoint structure to track positions with the same maturity date
#[derive(ScryptoSbor, Clone)]
pub struct Checkpoint {
    /// Start time of the checkpoint
    pub start_time: u64,
    /// Share price at the time the checkpoint was minted
    pub share_price: Decimal,
    /// Amount of longs in checkpoint (yl)
    pub long_positions: Decimal,
    /// Amount of shorts in checkpoint (ys)
    pub short_positions: Decimal,
    /// Average maturity of long positions (tl)
    pub avg_long_maturity: Decimal,
    /// Average maturity of short positions (ts)
    pub avg_short_maturity: Decimal,
    /// Whether checkpoint is minted
    pub is_minted: bool,
}

/// Structure to represent the pool's state for external queries
#[derive(ScryptoSbor, Clone)]
pub struct PoolState {
    /// Share reserves (z)
    pub share_reserves: Decimal,
    /// Bond reserves (y)
    pub bond_reserves: Decimal,
    /// Zeta adjustment (ζ)
    pub zeta_adjustment: Decimal,
    /// Effective share reserves (ze = z - ζ)
    pub effective_share_reserves: Decimal,
    /// Current share price (c)
    pub share_price: Decimal,
    /// Current spot rate
    pub spot_rate: Decimal,
    /// Total active LP shares
    pub active_lp_shares_address: Decimal,
    /// Total withdrawal shares
    pub withdrawal_shares_address: Decimal,
    /// Total ready withdrawal shares
    pub ready_withdrawal_shares_address: Decimal,
    /// Zombie share reserves
    pub zombie_share_reserves: Decimal,
    /// Zombie base reserves
    pub zombie_base_reserves: Decimal,
    /// Current checkpoint ID
    pub current_checkpoint: u64,
}

/// Long position data stored in NFT
#[derive(ScryptoSbor, ManifestSbor, NonFungibleData)]
pub struct LongPosition {
    /// Face value of the long position (Δy)
    pub face_value: Decimal,
    /// Checkpoint when position was opened
    pub checkpoint: u64,
    /// Time when position was opened
    pub open_time: u64,
    /// Time when position matures
    pub maturity_time: u64,
}

/// Short position data stored in NFT
#[derive(ScryptoSbor, ManifestSbor, NonFungibleData)]
pub struct ShortPosition {
    /// Face value of the short position (Δy)
    pub face_value: Decimal,
    /// Checkpoint when position was opened
    pub checkpoint: u64,
    /// Time when position was opened
    pub open_time: u64,
    /// Time when position matures
    pub maturity_time: u64,
    /// Share price when position was opened (c0)
    pub initial_share_price: Decimal,
}

/// Complete data structure for a Hyperdrive pool
/// Used in the KeyValueStore to manage multiple pools
#[derive(ScryptoSbor)]
pub struct HyperdrivePoolData {
    // State variables
    pub share_reserves: Decimal,         // z: Share reserves
    pub bond_reserves: Decimal,          // y: Bond reserves
    pub zeta_adjustment: Decimal,        // ζ: Zeta adjustment
    
    // Yield source
    pub share_price: Decimal,            // c: Current share price
    
    // Resource addresses
    pub bond_resource_address: ResourceAddress,
    pub active_lp_shares_address: ResourceAddress,
    pub withdrawal_shares_address: ResourceAddress,
    pub ready_withdrawal_shares_address: ResourceAddress,
    pub long_positions_resource: ResourceAddress,
    pub short_positions_resource: ResourceAddress,
    
    // Checkpoints
    pub checkpoints: HashMap<u64, Checkpoint>,
    pub checkpoint_duration: u64,        // dc: Checkpoint duration
    pub position_duration: u64,          // Full term duration
    pub current_checkpoint: u64,         // Current checkpoint ID
    
    // Fees
    pub new_bond_fee: Decimal,           // ϕn: Fee for newly minted bonds
    pub matured_bond_fee: Decimal,       // ϕm: Fee for matured bonds
    pub governance_fee: Decimal,         // ϕg: Governance fee portion
    pub zombie_governance_fee: Decimal,  // ϕg,zombie: Zombie interest governance fee
    
    // Minimum reserves
    pub min_share_reserves: Decimal,     // zmin: Minimum share reserves
    
    // Zombie reserves
    pub zombie_share_reserves: Decimal,  // zzombie: Zombie share reserves
    pub zombie_base_reserves: Decimal,   // xzombie: Zombie base reserves
    
    // Vaults
    pub yield_source_vault: Vault,       // Holds the base tokens
    pub bond_vault: Vault,               // Holds bond tokens
    pub active_lp_vault: Vault,          // Holds active LP tokens
    pub withdrawal_vault: Vault,         // Holds withdrawal tokens
    pub ready_withdrawal_vault: Vault,   // Holds ready withdrawal tokens
    pub governance_vault: Vault,         // Holds governance fees
    
    // Minter badge for this specific pool
    pub minter_badge: Vault,             // Holds the minter badge for resource operations
}