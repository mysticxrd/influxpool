use scrypto::prelude::*;

// Import all modules for use in the blueprint
use crate::types::*;
use crate::helpers::*;
use crate::events::*;
use crate::dex::*;
use crate::liquidity::*;

#[blueprint]
mod hyperdrive_pool {
    enable_method_auth! {
        methods {
            create_pool => PUBLIC;
            open_long => PUBLIC;
            close_long => PUBLIC;
            open_short => PUBLIC;
            close_short => PUBLIC;
            add_liquidity => PUBLIC;
            remove_liquidity => PUBLIC;
            get_pool_state => PUBLIC;
            effective_share_reserves => PUBLIC;
            get_spot_rate => PUBLIC;
            update_share_price => PUBLIC;
            get_pool_count => PUBLIC;
            withdraw_governance_fees => restrict_to: [OWNER];
        }
    }

    struct HyperdrivePool {
        // Global configuration 
        yield_source: ResourceAddress,   // Base token (yield-bearing asset)
        admin_badge: ResourceAddress,    // For admin operations

        // Pool management
        pool_counter: u64,               // Counter for pool IDs
        pools: KeyValueStore<u64, ComponentAddress>, // Pool registry
        
        // Current active pool state (only one pool per component for now)
        // This can be extended to support multiple pools later
        is_initialized: bool,            // Whether a pool has been created
        
        // Pool state variables (moved from instance)
        pool_id: u64,
        share_reserves: Decimal,         // z: Share reserves
        bond_reserves: Decimal,          // y: Bond reserves
        zeta_adjustment: Decimal,        // ζ: Zeta adjustment
        share_price: Decimal,            // c: Current share price
        
        // Bond token
        bond_resource_address: Option<ResourceAddress>,
        
        // LP tokens
        active_lp_shares_address: Option<ResourceAddress>,
        withdrawal_shares_address: Option<ResourceAddress>,
        ready_withdrawal_shares_address: Option<ResourceAddress>,
        
        // Checkpoints
        checkpoints: HashMap<u64, Checkpoint>,
        checkpoint_duration: u64,        // dc: Checkpoint duration
        position_duration: u64,          // Full term duration
        current_checkpoint: u64,         // Current checkpoint ID
        
        // Fees
        new_bond_fee: Decimal,           // ϕn: Fee for newly minted bonds
        matured_bond_fee: Decimal,       // ϕm: Fee for matured bonds
        governance_fee: Decimal,         // ϕg: Governance fee portion
        zombie_governance_fee: Decimal,  // ϕg,zombie: Zombie interest governance fee
        
        // Minimum reserves
        min_share_reserves: Decimal,     // zmin: Minimum share reserves
        
        // Zombie reserves
        zombie_share_reserves: Decimal,  // zzombie: Zombie share reserves
        zombie_base_reserves: Decimal,   // xzombie: Zombie base reserves
        
        // Vaults
        yield_source_vault: Option<Vault>,       // Holds the base tokens
        bond_vault: Option<Vault>,               // Holds bond tokens
        active_lp_vault: Option<Vault>,          // Holds active LP tokens
        withdrawal_vault: Option<Vault>,         // Holds withdrawal tokens
        ready_withdrawal_vault: Option<Vault>,   // Holds ready withdrawal tokens
        governance_vault: Option<Vault>,         // Holds governance fees
        
        // Position tracking
        long_positions_resource: Option<ResourceAddress>,  // NFT for long positions
        short_positions_resource: Option<ResourceAddress>, // NFT for short positions
    }
    
    impl HyperdrivePool {
        /// Creates the component with minimal parameters 
        /// 
        /// # Arguments
        /// * `yield_source` - Resource address of the yield-bearing asset
        /// * `admin_badge` - Badge for administrative operations
        /// 
        /// # Returns
        /// * Global<HyperdrivePool> - The Hyperdrive AMM component
        pub fn instantiate_dex(yield_source: ResourceAddress, admin_badge: ResourceAddress) -> Global<HyperdrivePool> {
            Self {
                yield_source,
                admin_badge,
                pool_counter: 0,
                pools: KeyValueStore::new(),
                is_initialized: false,
                
                // Initialize with default/empty values
                pool_id: 0,
                share_reserves: Decimal::ZERO,
                bond_reserves: Decimal::ZERO,
                zeta_adjustment: Decimal::ZERO,
                share_price: Decimal::ONE,
                
                bond_resource_address: None,
                active_lp_shares_address: None,
                withdrawal_shares_address: None,
                ready_withdrawal_shares_address: None,
                
                checkpoints: HashMap::new(),
                checkpoint_duration: 0,
                position_duration: 0,
                current_checkpoint: 0,
                
                new_bond_fee: Decimal::ZERO,
                matured_bond_fee: Decimal::ZERO,
                governance_fee: Decimal::ZERO,
                zombie_governance_fee: Decimal::ZERO,
                
                min_share_reserves: Decimal::ZERO,
                zombie_share_reserves: Decimal::ZERO,
                zombie_base_reserves: Decimal::ZERO,
                
                yield_source_vault: None,
                bond_vault: None,
                active_lp_vault: None,
                withdrawal_vault: None,
                ready_withdrawal_vault: None,
                governance_vault: None,
                
                long_positions_resource: None,
                short_positions_resource: None,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(admin_badge))))
            .globalize()
        }
        
        /// Creates a new Hyperdrive pool with full configuration 
        
        /// # Arguments
        /// * `checkpoint_duration` - Duration of each checkpoint in seconds
        /// * `position_duration` - Duration of positions in seconds
        /// * `new_bond_fee` - Fee percentage for newly minted bonds (ϕn)
        /// * `matured_bond_fee` - Fee percentage for matured bonds (ϕm)
        /// * `governance_fee` - Governance fee portion (ϕg)
        /// * `zombie_governance_fee` - Zombie interest governance fee (ϕg,zombie)
        /// * `min_share_reserves` - Minimum share reserves (zmin)
        /// * `initial_liquidity` - Initial liquidity to seed the pool
        
        /// # Returns
        /// * Bucket - Initial LP tokens
        pub fn create_pool(
            &mut self,
            checkpoint_duration: u64,
            position_duration: u64,
            new_bond_fee: Decimal,
            matured_bond_fee: Decimal,
            governance_fee: Decimal,
            zombie_governance_fee: Decimal,
            min_share_reserves: Decimal,
            initial_liquidity: Bucket,
        ) -> Bucket {
            // Ensure only one pool per component for now (can be extended later)
            assert!(!self.is_initialized, "Pool already initialized");
            
            // Validate parameters
            validate_durations(checkpoint_duration, position_duration);
            validate_fee(new_bond_fee, "New bond fee");
            validate_fee(matured_bond_fee, "Matured bond fee");
            validate_fee(governance_fee, "Governance fee");
            validate_fee(zombie_governance_fee, "Zombie governance fee");
            validate_resource_address(initial_liquidity.resource_address(), self.yield_source, "initial liquidity");
            validate_liquidity_amount(initial_liquidity.amount(), min_share_reserves);

            // Increment pool counter and set pool ID
            self.pool_counter += 1;
            self.pool_id = self.pool_counter;
            
            // Get component address for authorization
            let component_address = Runtime::global_address();
            let global_component_caller_badge = 
                NonFungibleGlobalId::global_caller_badge(component_address);
                
            // Create bond resource with component as minter
            let bond_resource = ResourceBuilder::new_fungible(OwnerRole::None)
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata(metadata!(
                    init {
                        "name" => format!("Hyperdrive Bond Token - Pool {}", self.pool_id), locked;
                        "symbol" => format!("HDB-{}", self.pool_id), locked;
                    }
                ))
                .mint_roles(mint_roles! {
                    minter => rule!(require(global_component_caller_badge.clone()));
                    minter_updater => rule!(deny_all);
                })
                .burn_roles(burn_roles! {
                    burner => rule!(require(global_component_caller_badge.clone()));
                    burner_updater => rule!(deny_all);
                })
                .create_with_no_initial_supply();

            self.bond_resource_address = Some(bond_resource.address());
                
            // Create LP token resources with component as minter
            let active_lp_shares = ResourceBuilder::new_fungible(OwnerRole::None)
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata(metadata!(
                    init {
                        "name" => format!("Hyperdrive LP Token - Pool {}", self.pool_id), locked;
                        "symbol" => format!("HDLP-{}", self.pool_id), locked;
                    }
                ))
                .mint_roles(mint_roles! {
                    minter => rule!(require(global_component_caller_badge.clone()));
                    minter_updater => rule!(deny_all);
                })
                .burn_roles(burn_roles! {
                    burner => rule!(require(global_component_caller_badge.clone()));
                    burner_updater => rule!(deny_all);
                })
                .create_with_no_initial_supply();

            self.active_lp_shares_address = Some(active_lp_shares.address());
                
            let withdrawal_shares = ResourceBuilder::new_fungible(OwnerRole::None)
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata(metadata!(
                    init {
                        "name" => format!("Hyperdrive Withdrawal Token - Pool {}", self.pool_id), locked;
                        "symbol" => format!("HDWD-{}", self.pool_id), locked;
                    }
                ))
                .mint_roles(mint_roles! {
                    minter => rule!(require(global_component_caller_badge.clone()));
                    minter_updater => rule!(deny_all);
                })                
                .burn_roles(burn_roles! {
                    burner => rule!(require(global_component_caller_badge.clone()));
                    burner_updater => rule!(deny_all);
                })
                .create_with_no_initial_supply();

            self.withdrawal_shares_address = Some(withdrawal_shares.address());
                
            let ready_withdrawal_shares = ResourceBuilder::new_fungible(OwnerRole::None)
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata(metadata! ( 
                    init{
                        "name" => format!("Hyperdrive Ready Withdrawal Token - Pool {}", self.pool_id), locked;
                        "symbol" => format!("HDRW-{}", self.pool_id), locked;
                    }
                ))
                .mint_roles(mint_roles! {
                    minter => rule!(require(global_component_caller_badge.clone()));
                    minter_updater => rule!(deny_all);
                })
                .burn_roles(burn_roles! {
                    burner => rule!(require(global_component_caller_badge.clone()));
                    burner_updater => rule!(deny_all);
                })                
                .create_with_no_initial_supply();

            self.ready_withdrawal_shares_address = Some(ready_withdrawal_shares.address());

            // Create position NFT resources with component as minter
            let long_positions = ResourceBuilder::new_ruid_non_fungible::<LongPosition>(OwnerRole::None)
                .metadata(metadata!(
                    init {
                        "name" => format!("Hyperdrive Long Position - Pool {}", self.pool_id), locked;
                        "symbol" => format!("HDLG-{}", self.pool_id), locked;
                    }
                ))
                .mint_roles(mint_roles! {
                    minter => rule!(require(global_component_caller_badge.clone()));
                    minter_updater => rule!(deny_all);
                })
                .burn_roles(burn_roles! {
                    burner => rule!(require(global_component_caller_badge.clone()));
                    burner_updater => rule!(deny_all);
                }) 
                .non_fungible_data_update_roles(non_fungible_data_update_roles! {
                    non_fungible_data_updater => rule!(require(global_component_caller_badge.clone()));
                    non_fungible_data_updater_updater => rule!(deny_all);
                })
                .create_with_no_initial_supply();

            self.long_positions_resource = Some(long_positions.address());

            let short_positions = ResourceBuilder::new_ruid_non_fungible::<ShortPosition>(OwnerRole::None)
                .metadata(metadata!(
                    init {
                        "name" => format!("Hyperdrive Short Position - Pool {}", self.pool_id), locked;
                        "symbol" => format!("HDSH-{}", self.pool_id), locked;
                    }
                ))
                .mint_roles(mint_roles! {
                    minter => rule!(require(global_component_caller_badge.clone()));
                    minter_updater => rule!(deny_all);
                })
                .burn_roles(burn_roles! {
                    burner => rule!(require(global_component_caller_badge.clone()));
                    burner_updater => rule!(deny_all);
                }) 
                .non_fungible_data_update_roles(non_fungible_data_update_roles! {
                    non_fungible_data_updater => rule!(require(global_component_caller_badge.clone()));
                    non_fungible_data_updater_updater => rule!(deny_all);
                })
                .create_with_no_initial_supply();

            self.short_positions_resource = Some(short_positions.address());
            
            // Set pool parameters
            self.checkpoint_duration = checkpoint_duration;
            self.position_duration = position_duration;
            self.new_bond_fee = new_bond_fee;
            self.matured_bond_fee = matured_bond_fee;
            self.governance_fee = governance_fee;
            self.zombie_governance_fee = zombie_governance_fee;
            self.min_share_reserves = min_share_reserves;
            
            // Calculate initial share reserves (minus minimum reserves)
            let initial_amount = initial_liquidity.amount();
            self.share_reserves = initial_amount - min_share_reserves;
            self.bond_reserves = Decimal::ZERO;
            self.zeta_adjustment = Decimal::ZERO;
            self.share_price = Decimal::ONE; // Initial share price
            
            // Initialize checkpoints
            self.current_checkpoint = initialize_first_checkpoint(
                &mut self.checkpoints, 
                checkpoint_duration, 
                Decimal::ONE
            );
            
            // Initialize vaults
            self.yield_source_vault = Some(Vault::with_bucket(initial_liquidity));
            self.bond_vault = Some(Vault::new(self.bond_resource_address.unwrap()));
            self.active_lp_vault = Some(Vault::new(self.active_lp_shares_address.unwrap()));
            self.withdrawal_vault = Some(Vault::new(self.withdrawal_shares_address.unwrap()));
            self.ready_withdrawal_vault = Some(Vault::new(self.ready_withdrawal_shares_address.unwrap()));
            self.governance_vault = Some(Vault::new(self.yield_source));
            
            // Mark as initialized
            self.is_initialized = true;
            
            // Register pool in the registry
            self.pools.insert(self.pool_id, component_address);
            
            // Mint initial LP tokens
            let initial_lp_tokens = active_lp_shares.mint(self.share_reserves).into();
            
            initial_lp_tokens
        }
        
        /// Opens a long position
        pub fn open_long(&mut self, base_tokens: Bucket) -> Bucket {
            assert!(self.is_initialized, "Pool not initialized");
            
            open_long_position(
                base_tokens,
                &mut self.share_reserves,
                &mut self.bond_reserves,
                &mut self.zeta_adjustment,
                self.share_price,
                &mut self.checkpoints,
                &mut self.current_checkpoint,
                self.checkpoint_duration,
                self.position_duration,
                self.new_bond_fee,
                self.governance_fee,
                self.long_positions_resource.unwrap(),
                self.yield_source,
                self.governance_vault.as_mut().unwrap(),
                self.yield_source_vault.as_mut().unwrap()
            )
        }
        
        /// Closes a long position
        pub fn close_long(&mut self, position_nft: Bucket) -> Bucket {
            assert!(self.is_initialized, "Pool not initialized");
            
            close_long_position(
                position_nft,
                &mut self.share_reserves,
                &mut self.bond_reserves,
                &mut self.zeta_adjustment,
                self.share_price,
                &mut self.checkpoints,
                &mut self.current_checkpoint,
                self.checkpoint_duration,
                self.new_bond_fee,
                self.matured_bond_fee,
                self.governance_fee,
                self.long_positions_resource.unwrap(),
                self.governance_vault.as_mut().unwrap(),
                self.yield_source_vault.as_mut().unwrap()
            )
        }
        
        /// Opens a short position
        pub fn open_short(&mut self, base_tokens: Bucket, face_value: Decimal) -> (Bucket, Bucket) {
            assert!(self.is_initialized, "Pool not initialized");
            
            open_short_position(
                base_tokens,
                face_value,
                &mut self.share_reserves,
                &mut self.bond_reserves,
                &mut self.zeta_adjustment,
                self.share_price,
                &mut self.checkpoints,
                &mut self.current_checkpoint,
                self.checkpoint_duration,
                self.position_duration,
                self.new_bond_fee,
                self.governance_fee,
                self.short_positions_resource.unwrap(),
                self.yield_source,
                self.governance_vault.as_mut().unwrap(),
                self.yield_source_vault.as_mut().unwrap()
            )
        }
        
        /// Closes a short position
        pub fn close_short(&mut self, position_nft: Bucket) -> Bucket {
            assert!(self.is_initialized, "Pool not initialized");
            
            close_short_position(
                position_nft,
                &mut self.share_reserves,
                &mut self.bond_reserves,
                &mut self.zeta_adjustment,
                self.share_price,
                &mut self.checkpoints,
                &mut self.current_checkpoint,
                self.checkpoint_duration,
                self.new_bond_fee,
                self.matured_bond_fee,
                self.governance_fee,
                self.short_positions_resource.unwrap(),
                self.governance_vault.as_mut().unwrap(),
                self.yield_source_vault.as_mut().unwrap()
            )
        }
        
        /// Adds liquidity to the pool
        pub fn add_liquidity(&mut self, base_tokens: Bucket) -> Bucket {
            assert!(self.is_initialized, "Pool not initialized");
            
            add_liquidity(
                base_tokens,
                &mut self.share_reserves,
                &mut self.bond_reserves,
                self.share_price,
                self.active_lp_vault.as_ref().unwrap().amount(),
                self.active_lp_shares_address.unwrap(),
                self.yield_source,
                self.yield_source_vault.as_mut().unwrap()
            )
        }
        
        /// Removes liquidity from the pool
        pub fn remove_liquidity(&mut self, lp_tokens: Bucket) -> (Bucket, Bucket) {
            assert!(self.is_initialized, "Pool not initialized");
            
            remove_liquidity(
                lp_tokens,
                &mut self.share_reserves,
                &mut self.bond_reserves,
                self.share_price,
                self.active_lp_vault.as_ref().unwrap().amount(),
                self.min_share_reserves,
                &self.checkpoints,
                self.current_checkpoint,
                self.checkpoint_duration,
                self.position_duration,
                self.active_lp_shares_address.unwrap(),
                self.withdrawal_shares_address.unwrap(),
                self.yield_source_vault.as_mut().unwrap()
            )
        }
        
        /// Gets the current pool state
        pub fn get_pool_state(&self) -> PoolState {
            assert!(self.is_initialized, "Pool not initialized");
            
            get_pool_state(
                self.share_reserves,
                self.bond_reserves,
                self.zeta_adjustment,
                self.share_price,
                self.active_lp_vault.as_ref().unwrap().amount(),
                self.withdrawal_vault.as_ref().unwrap().amount(),
                self.ready_withdrawal_vault.as_ref().unwrap().amount(),
                self.zombie_share_reserves,
                self.zombie_base_reserves,
                self.current_checkpoint
            )
        }
        
        /// Gets the effective share reserves
        pub fn effective_share_reserves(&self) -> Decimal {
            assert!(self.is_initialized, "Pool not initialized");
            calculate_effective_share_reserves(self.share_reserves, self.zeta_adjustment)
        }
        
        /// Gets the current spot rate
        pub fn get_spot_rate(&self) -> Decimal {
            assert!(self.is_initialized, "Pool not initialized");
            let effective_shares = self.effective_share_reserves();
            calculate_spot_rate(effective_shares, self.bond_reserves, self.share_price)
        }
        
        /// Updates the share price from the yield source
        pub fn update_share_price(&mut self, new_share_price: Decimal) {
            assert!(self.is_initialized, "Pool not initialized");
            update_share_price_from_yield_source(new_share_price, &mut self.share_price);
        }
        
        /// Withdraws governance fees (admin only)
        pub fn withdraw_governance_fees(&mut self, auth: Proof) -> Bucket {
            assert!(self.is_initialized, "Pool not initialized");
            withdraw_governance_fees(self.admin_badge, auth, self.governance_vault.as_mut().unwrap())
        }
        
        /// Gets the total number of pools created
        pub fn get_pool_count(&self) -> u64 {
            self.pool_counter
        }
    }
}