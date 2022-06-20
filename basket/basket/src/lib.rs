use scrypto::prelude::*;

// FR05T8YTE

// automated fund managed by users staking samusky to a token the fund should buy 
// basket token represents a part ownership of the fund 
// stake losers pay stake winners
// percent of new investments go to stakers

// import fake amm pool blueprint as a generic interface
import! { r#"
{
    "package_address": "",
    "blueprint_name": "AmmPool",
    "functions": [],
    "methods": [
        {
        "name": "swap",
        "mutability": "Mutable",
        "inputs": [
            {
            "type": "Custom",
            "name": "Bucket",
            "generics": []
            }
        ],
        "output": {
            "type": "Custom",
            "name": "Bucket",
            "generics": []
        }
        },
        {
        "name": "get_pair",
        "mutability": "Immutable",
        "inputs": [],
        "output": {
            "type": "Tuple",
            "elements": [
            {
                "type": "Custom",
                "name": "ResourceAddress",
                "generics": []
            },
            {
                "type": "Custom",
                "name": "ResourceAddress",
                "generics": []
            }
            ]
        }
        },
        {
        "name": "get_price",
        "mutability": "Immutable",
        "inputs": [],
        "output": {
            "type": "Custom",
            "name": "Decimal",
            "generics": []
        }
        }
    ]
}
"# }

// nft that represents a stake
#[derive(NonFungibleData)]
pub struct StakeReceipt {
    pool_address: ComponentAddress,     // pool of token staked to, acts as an identifer for the token
    #[scrypto(mutable)]
    amount: Decimal,                    // amount initially staked or after unstake amount to be collected
    #[scrypto(mutable)]
    weight: Decimal,                    // amount of weight the stake has; weight / total_stake_weight * total_stake_amount = stake_amount
    #[scrypto(mutable)]
    status: u8,                         // code used to track current state of stake
}

// StakeReceipt status codes
const STAKING: u8 = 0;
const STAKED: u8 = 1;
const UNSTAKING: u8 = 2;
const UNSTAKED: u8 = 3;

blueprint! {
    struct Basket {
        basket_token_address: ResourceAddress,                      // basket token that represents a part ownership of the fund 
        samusky_token_address: ResourceAddress,                     // samusky token that is used for staking
        samusky_amm_pool: ComponentAddress,                         // amm pool used to buy samusky using fees
        stake_receipt_address: ResourceAddress,                     // nft that represents a stake
        internal_badge: Vault,                                      // badge used for all internal permission (minting, burning, etc.)
        radix_reserve_percent: Decimal,                             // percent of investments value to be kept in resevere as xrd
        fee_percent: Decimal,                                       // percent of buying xrd transfered to stakers
        radix_tokens: Vault,                                        // radix reserve tokens
        samusky_tokens: Vault,                                      // basket tokens that are in the process of staking or have been unstake and not yet collected
        amm_pools: Vec<ComponentAddress>,                           // list of pools used to swap
        investments: Vec<Vault>,                                    // list of vaults containing tokens that have been bought by the fund
        stakes: Vec<(Vault, Decimal)>,                              // list of (stake, total_stake_weight)
        last_prices: Vec<Decimal>,                                  // list of prices at last rebalance
        staking_queue: HashSet<NonFungibleId>,                      // staking queue that is processed at rebalance
        unstaking_queue: HashSet<NonFungibleId>,                    // unstaing queue that is processed at rebalance
    }

    impl Basket {
        // instantiates a fund
        // returns (fund_address, admin_badge)
        // admin_badge is used to whitelist pools for tokens that can then be staked to and change other key variables
        pub fn instantiate_fund(samusky_token_address: ResourceAddress, samusky_amm_pool: ComponentAddress, radix_reserve_percent: Decimal, fee_percent: Decimal) -> (ComponentAddress, Bucket) {
            let pool: AmmPool = samusky_amm_pool.into();
            let pair: (ResourceAddress, ResourceAddress) = pool.get_pair();
            
            assert!(
                pair.0 == samusky_token_address && pair.1 == RADIX_TOKEN,
                "First token in pair must be samusky and second token in pair must be radix."
            );
            
            let admin_badge: Bucket = ResourceBuilder::new_fungible()
            .metadata("name", "Basket Admin Badge")
            .metadata("symbol", "ADMIN")    
            .initial_supply(1);

            let internal_badge: Bucket = ResourceBuilder::new_fungible()
                .initial_supply(1);

            let basket_token_address: ResourceAddress = ResourceBuilder::new_fungible()
                .metadata("name", "Basket")
                .metadata("symbol", "BKT")
                .mintable(rule!(require(internal_badge.resource_address())), LOCKED)
                .burnable(rule!(require(internal_badge.resource_address())), LOCKED)
                .no_initial_supply();

            let stake_receipt_address: ResourceAddress = ResourceBuilder::new_non_fungible()
                .metadata("name", "Basket Stake Receipt")
                .metadata("symbol", "BKTsr")
                .mintable(rule!(require(internal_badge.resource_address())), LOCKED)
                .burnable(rule!(require(internal_badge.resource_address())), LOCKED)
                .updateable_non_fungible_data(rule!(require(internal_badge.resource_address())), LOCKED)
                .no_initial_supply();

            let auth: AccessRules = AccessRules::new()
                .method("add_investment", rule!(require(admin_badge.resource_address())))
                .method("remove_investment", rule!(require(admin_badge.resource_address())))
                .method("set_fee_percent", rule!(require(admin_badge.resource_address())))
                .method("set_radix_reserve_percent", rule!(require(admin_badge.resource_address())))
                .default(rule!(allow_all));

            (
                Self {
                    basket_token_address: basket_token_address,
                    samusky_token_address: samusky_token_address,
                    samusky_amm_pool: samusky_amm_pool,
                    stake_receipt_address: stake_receipt_address,
                    internal_badge: Vault::with_bucket(internal_badge),
                    radix_reserve_percent: radix_reserve_percent,
                    fee_percent: fee_percent,
                    radix_tokens: Vault::new(RADIX_TOKEN),
                    samusky_tokens: Vault::new(samusky_token_address),
                    amm_pools: Vec::new(),
                    investments: Vec::new(),
                    stakes: Vec::new(),
                    last_prices: Vec::new(),
                    staking_queue: HashSet::new(),
                    unstaking_queue: HashSet::new(),
                }
                .instantiate()
                .add_access_check(auth)
                .globalize(),
                admin_badge
            )
        }

        // whitelists a investment token for the fund
        // requires admin_badge to call
        pub fn add_investment(&mut self, pool_address: ComponentAddress) {
            assert!(
                !self.amm_pools.contains(&pool_address),
                "Investment is already part of the fund."
            );
            
            let pool: AmmPool = pool_address.into();
            let pair: (ResourceAddress, ResourceAddress) = pool.get_pair();
            
            assert!(
                pair.1 == RADIX_TOKEN,
                "Second token in pair must be radix."
            );

            self.amm_pools.push(pool_address);
            self.investments.push(Vault::new(pair.0));
            self.stakes.push((Vault::new(self.samusky_token_address), dec!(0)));
            self.last_prices.push(dec!(-1));
        }

        // changes the radix reserve percent for the fund
        // requires admin_badge to call
        pub fn set_radix_reserve_percent(&mut self, radix_reserve_percent: Decimal) {
            assert!(
                radix_reserve_percent >= dec!(0),
                "Invalid radix_reserve_percent value."
            );

            self.radix_reserve_percent = radix_reserve_percent;
        }

        // changes the fee percent for the fund
        // requires admin_badge to call
        pub fn set_fee_percent(&mut self, fee_percent: Decimal) {
            assert!(
                fee_percent >= dec!(0) && fee_percent <= dec!(100),
                "Invalid fee_percent value."
            );

            self.fee_percent = fee_percent;
        }
        
        // trades radix tokens for basket tokens
        // fund buys investment tokens using the radix in proportion to stakes plus sets aside radix for the reserve
        // returns basket tokens
        pub fn buy(&mut self, mut radix_tokens: Bucket) -> Bucket {
            let mut amount: Decimal = radix_tokens.amount();
            let stake_denominator: Decimal = self.get_stake_denominator();

            if stake_denominator != dec!(0) {
                let total_stake: Decimal = self.get_total_stake();
                let samusky_pool: AmmPool = self.samusky_amm_pool.into();
                let fee_amount: Decimal = amount * self.fee_percent / dec!(100);
                let mut fee: Bucket = samusky_pool.swap(radix_tokens.take(fee_amount));
                amount = radix_tokens.amount();

                for i in 0..self.investments.len() {
                    let calculated_fee_share: Decimal = fee_amount * self.stakes[i].0.amount() / total_stake;
                    let fee_share: Decimal = if calculated_fee_share > fee.amount() {
                        fee.amount()
                    } else {
                        calculated_fee_share
                    };
                    
                    self.stakes[i].0.put(fee.take(fee_share));
                }
                self.stakes[0].0.put(fee);

                for i in 0..self.investments.len() {
                    let mut buy_amount: Decimal = amount * self.stakes[i].0.amount() / stake_denominator;
                    if buy_amount > radix_tokens.amount() {
                        buy_amount = radix_tokens.amount();
                    }
                    
                    let pool: AmmPool = self.amm_pools[i].into();
                    let tokens: Bucket = pool.swap(radix_tokens.take(buy_amount));
                    self.investments[i].put(tokens);
                }
            }
            self.radix_tokens.put(radix_tokens);
            
            let basket_token_manager: &ResourceManager = borrow_resource_manager!(self.basket_token_address);
            let total_supply = basket_token_manager.total_supply();

            let mint_amount: Decimal = if total_supply == dec!(0) {
                self.get_total_value()
            } else {
                amount * total_supply / (self.get_total_value() - amount)
            };

            let basket_tokens: Bucket = self.internal_badge.authorize(|| {
                basket_token_manager.mint(mint_amount)
            });

            basket_tokens
        }

        // trades basket tokens for radix tokens
        // fund sells investment tokens for radix in proportion to stakes plus take radix from the reserve
        // returns radix tokens
        pub fn sell(&mut self, basket_tokens: Bucket) -> Bucket {
            let basket_token_manager: &ResourceManager = borrow_resource_manager!(self.basket_token_address);
            let amount_ownership: Decimal = basket_tokens.amount() / basket_token_manager.total_supply();
            let mut radix_tokens: Bucket = self.radix_tokens.take(self.radix_tokens.amount() * amount_ownership);

            for i in 0..self.investments.len() {
                let pool: AmmPool = self.amm_pools[i].into();
                let sell_amount: Decimal = self.investments[i].amount() * amount_ownership;
                let tokens: Bucket = pool.swap(self.investments[i].take(sell_amount));
                radix_tokens.put(tokens);
            }
            
            self.internal_badge.authorize(|| {
                basket_tokens.burn()
            });

            radix_tokens
        }

        // starts the process of staking samusky tokens for a investment token
        // staking is processed at next rebalance
        // returns a stake receipt nft
        pub fn stake(&mut self, samusky_tokens: Bucket, pool_address: ComponentAddress) -> Bucket {
            let stake_receipt_manager: &ResourceManager = borrow_resource_manager!(self.stake_receipt_address);

            assert!(
                self.amm_pools.contains(&pool_address),
                "Can not stake for a token that does not have an approved pool."
            );

            let id: NonFungibleId = NonFungibleId::random();
            let stake_receipt: Bucket = self.internal_badge.authorize(|| {
                stake_receipt_manager.mint_non_fungible(
                    &id,
                    StakeReceipt {
                        pool_address: pool_address,
                        amount: samusky_tokens.amount(),
                        weight: dec!(0),
                        status: STAKING,
                    }
                )
            });

            self.samusky_tokens.put(samusky_tokens);
            self.staking_queue.insert(id);

            stake_receipt
        }

        // starts the process of unstaking stake receipts share of the tokens
        // unstaking is processed at the next rebalance
        // returns the updated stake receipt nft
        pub fn unstake(&mut self, stake_receipt: Bucket) -> Bucket {
            assert!(
                stake_receipt.resource_address() == self.stake_receipt_address,
                "Invalid stake receipt."
            );

            let stake_receipt_nft: NonFungible<StakeReceipt> = stake_receipt.non_fungible::<StakeReceipt>();

            match stake_receipt_nft.data().status {
                STAKING => {
                    panic!("Currently in the queue for staking. To cancel call cancel_staking.")
                },
                STAKED  => {
                    let mut stake_receipt_data: StakeReceipt = stake_receipt_nft.data();
                    stake_receipt_data.status = UNSTAKING;

                    self.internal_badge.authorize(|| {
                        stake_receipt_nft.update_data(stake_receipt_data);
                    });

                    self.unstaking_queue.insert(stake_receipt_nft.id());

                    stake_receipt
                },
                UNSTAKING  => {
                    panic!("Already unstaking.")
                },
                UNSTAKED  => {
                    panic!("Already unstaked. To collect tokens call collect_unstaked.")
                },
                _ => {
                    panic!("Invalid status.");
                },
            }
        }

        // cancels the process of staking if the tokens are not staked
        // burns the receipt
        // returns samusky tokens
        pub fn cancel_staking(&mut self, stake_receipt: Bucket) -> Bucket {
            assert!(
                stake_receipt.resource_address() == self.stake_receipt_address,
                "Invalid stake receipt."
            );

            let stake_receipt_nft: NonFungible<StakeReceipt> = stake_receipt.non_fungible::<StakeReceipt>();

            match stake_receipt_nft.data().status {
                STAKING => {
                    self.staking_queue.remove(&stake_receipt_nft.id());

                    let samusky_tokens: Bucket = self.samusky_tokens.take(stake_receipt_nft.data().amount);

                    self.internal_badge.authorize(|| {
                        stake_receipt.burn();
                    });

                    samusky_tokens
                },
                STAKED  => {
                    panic!("Currently staked. To unstake call unstake.")
                },
                UNSTAKING  => {
                    panic!("Currently in the queue for unstaking. Wait for unstaking to finish then call collect_unstake.")
                },
                UNSTAKED  => {
                    panic!("Currently unstaked. To collect tokens call collect_unstaked.")
                },
                _ => {
                    panic!("Invalid status.");
                },
            }
        }

        // collects unstaked samusky tokens
        // burns the stake receipt
        // returns samusky tokens
        pub fn collect_unstaked(&mut self, stake_receipt: Bucket) -> Bucket {
            assert!(
                stake_receipt.resource_address() == self.stake_receipt_address,
                "Invalid stake receipt."
            );

            let stake_receipt_nft: NonFungible<StakeReceipt> = stake_receipt.non_fungible::<StakeReceipt>();

            match stake_receipt_nft.data().status {
                STAKING => {
                    panic!("Currently in the queue for staking. To cancel call cancel_staking.")
                },
                STAKED  => {
                    panic!("Currently staked. To unstake call unstake.")
                },
                UNSTAKING  => {
                    panic!("Currently in the queue for unstaking. Wait for unstaking to finish then call collect_unstake again.")
                },
                UNSTAKED  => {
                    let samusky_tokens: Bucket = self.samusky_tokens.take(stake_receipt_nft.data().amount);

                    self.internal_badge.authorize(|| {
                        stake_receipt.burn();
                    });

                    samusky_tokens
                },
                _ => {
                    panic!("Invalid status.");
                },
            }
        }

        // updates stakes based on token performance in proportion to deviation from mean
        // rebalances the fund according to current investment token values in proportion to stake amounts
        // processes staking and unstaking
        pub fn rebalance(&mut self) {
            let stake_denominator: Decimal = self.get_stake_denominator();

            if stake_denominator != dec!(0) {
                let prices: Vec<Decimal> = self.get_prices();
                let total_value: Decimal = self.get_total_value();
                let amounts: Vec<Decimal> = self.get_amounts();
                let changes: Vec<Decimal> = self.get_mean_adjusted_changes();
            
                let mut samusky_tokens: Bucket = Bucket::new(self.samusky_token_address);

                for i in 0..self.investments.len() {
                    if changes[i] < dec!(0) {
                        let change_amount: Decimal = self.stakes[i].0.amount() * changes[i] * dec!(-1);
                        info!("Stake {} -{}", i, change_amount);
                        samusky_tokens.put(self.stakes[i].0.take(change_amount));
                    }
                }

                for i in 0..self.investments.len() {
                    if changes[i] > dec!(0) {
                        let calculated_change_amount: Decimal = self.stakes[i].0.amount() * changes[i];
                        let change_amount: Decimal = if calculated_change_amount > samusky_tokens.amount() {
                            samusky_tokens.amount()
                        } else {
                            calculated_change_amount
                        };

                        info!("Stake {} {}", i, change_amount);
                        self.stakes[i].0.put(samusky_tokens.take(change_amount));
                    }
                }

                self.stakes[0].0.put(samusky_tokens);

                if total_value != dec!(0) {
                    let mut rebalance_amounts: Vec<Decimal> = Vec::new();

                    for i in 0..self.investments.len() {
                        let rebalance_amount: Decimal = if amounts[i] == dec!(0) {
                            total_value * self.stakes[i].0.amount() / stake_denominator
                        } else {
                            let value: Decimal =  prices[i] * amounts[i];
                            ((self.stakes[i].0.amount() / stake_denominator) - (value / total_value)) * value
                        };
                        info!("Rebalance {} {}", i, rebalance_amount);
                        rebalance_amounts.push(rebalance_amount);
                    }

                    for i in 0..self.investments.len() {
                        if rebalance_amounts[i] < dec!(0) {
                            let pool: AmmPool = self.amm_pools[i].into();
                            let radix_tokens: Bucket = pool.swap(self.investments[i].take(rebalance_amounts[i] / prices[i] * -1));
                            self.radix_tokens.put(radix_tokens);
                        }
                    }

                    for i in 0..self.investments.len() {
                        if rebalance_amounts[i] > dec!(0) {
                            let pool: AmmPool = self.amm_pools[i].into();
                            let tokens: Bucket = pool.swap(self.radix_tokens.take(rebalance_amounts[i]));
                            self.investments[i].put(tokens);
                        }
                    }
                }
            }

            if self.handle_staking() || self.handle_unstaking() {
                self.rebalance();
            } else {
                self.last_prices = self.get_prices();
            }
        }

        // processes staking for all stake receipts in the queue
        fn handle_staking(&mut self) -> bool {
            if self.staking_queue.len() == 0 {
                false
            } else {
                let mut map: HashMap<ComponentAddress, usize> = HashMap::new();
                
                for i in 0..self.investments.len() {
                    map.insert(self.amm_pools[i], i);
                }
                
                let stake_receipt_manager: &ResourceManager = borrow_resource_manager!(self.stake_receipt_address);
                
                for id in &self.staking_queue {
                    let mut stake_receipt_data: StakeReceipt = stake_receipt_manager.get_non_fungible_data(id);
                    let idx: usize = *map.get(&stake_receipt_data.pool_address).unwrap();
                    
                    let weight: Decimal = if self.stakes[idx].0.amount() == dec!(0) {
                        dec!(1)
                    } else {
                        (((stake_receipt_data.amount + self.stakes[idx].0.amount()) / self.stakes[idx].0.amount()) - 1) * self.stakes[idx].1
                    }; 
                    
                    self.stakes[idx].0.put(self.samusky_tokens.take(stake_receipt_data.amount));
                    self.stakes[idx].1 += weight;

                    stake_receipt_data.weight = weight;
                    stake_receipt_data.status = STAKED;

                    self.internal_badge.authorize(|| {
                        stake_receipt_manager.update_non_fungible_data(id, stake_receipt_data);
                    });
                }

                self.staking_queue = HashSet::new();
                true
            }
        }

        // processes unstaking for all the stake receipts in the queue
        fn handle_unstaking(&mut self) -> bool {
            if self.unstaking_queue.len() == 0 {
                false
            } else {
                let mut map: HashMap<ComponentAddress, usize> = HashMap::new();
                
                for i in 0..self.investments.len() {
                    map.insert(self.amm_pools[i], i);
                }
                
                let stake_receipt_manager: &ResourceManager = borrow_resource_manager!(self.stake_receipt_address);
                
                for id in &self.unstaking_queue {
                    let mut stake_receipt_data: StakeReceipt = stake_receipt_manager.get_non_fungible_data(id);
                    let idx: usize = *map.get(&stake_receipt_data.pool_address).unwrap();
                    
                    let calculated_amount: Decimal = self.stakes[idx].0.amount() * stake_receipt_data.weight / self.stakes[idx].1;
                    let amount: Decimal = if calculated_amount > self.stakes[idx].0.amount() {
                        self.stakes[idx].0.amount()
                    } else {
                        calculated_amount
                    };

                    self.samusky_tokens.put(self.stakes[idx].0.take(amount));
                    self.stakes[idx].1 -= stake_receipt_data.weight;

                    stake_receipt_data.amount = amount;
                    stake_receipt_data.status = UNSTAKED;

                    self.internal_badge.authorize(|| {
                        stake_receipt_manager.update_non_fungible_data(id, stake_receipt_data);
                    });
                }

                self.unstaking_queue = HashSet::new();
                true
            }
        }

        // returns a vector of prices in xrd for investment tokens
        pub fn get_prices(&self) -> Vec<Decimal> {
            let mut prices: Vec<Decimal> = Vec::new();
            
            for i in 0..self.investments.len() {
                let pool: AmmPool = self.amm_pools[i].into();
                
                let price: Decimal = pool.get_price();
                if price == dec!(0) {
                    prices.push(self.last_prices[i])
                } else {
                    prices.push(price);
                }
            }

            prices
        }

        // returns a vector of the owned amounts of each investment token
        pub fn get_amounts(&self) -> Vec<Decimal> {
            let mut amounts: Vec<Decimal> = Vec::new();
            
            for i in 0..self.investments.len() {
                amounts.push(self.investments[i].amount());
            }

            amounts
        }

        // requires amount of staked tokens > 0
        // returns vector of stake and price mean adjusted investment token price changes
        pub fn get_mean_adjusted_changes(&self) -> Vec<Decimal> {
            let prices: Vec::<Decimal> = self.get_prices();
            let mut sum: Decimal = dec!(0);
            
            for i in 0..self.investments.len() {
                sum += self.stakes[i].0.amount() * prices[i] / self.last_prices[i];
            }
            
            let mean: Decimal = sum / self.get_total_stake();
            let mut changes: Vec<Decimal> = Vec::new();

            for i in 0..self.investments.len() {
                changes.push(prices[i] / (self.last_prices[i] * mean) - 1);
            }

            changes
        }

        // return the total value of all assets in the fund in xrd
        pub fn get_total_value(&self) -> Decimal {
            let prices: Vec<Decimal> = self.get_prices();
            let amounts: Vec<Decimal> = self.get_amounts();
            let mut sum: Decimal = dec!(0);

            for i in 0..self.investments.len() {
                sum += prices[i] * amounts[i];
            }

            sum + self.radix_tokens.amount()
        }

        // returns the intrinsic value of a basket token in xrd
        pub fn get_value(&self) -> Decimal {
            let basket_token_manager: &ResourceManager = borrow_resource_manager!(self.basket_token_address);
            let total_supply = basket_token_manager.total_supply();

            if total_supply == dec!(0) {
                dec!(1)
            } else {
                self.get_total_value() / total_supply
            }
        }

        // returns total amount of staked tokens
        pub fn get_total_stake(&self) -> Decimal {
            let mut sum: Decimal = dec!(0);

            for i in 0..self.investments.len() {
                sum += self.stakes[i].0.amount();
            }

            sum
        }

        // returns the denominator used when calculating amount of investment token to buy based on stake
        pub fn get_stake_denominator(&self) -> Decimal {
            self.get_total_stake() * (self.radix_reserve_percent + 100) / dec!(100)
        }
    }
}