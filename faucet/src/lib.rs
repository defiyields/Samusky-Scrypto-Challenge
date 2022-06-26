//! This blueprint provides a faucet you can set up to shill your
//! project, help the needy, or whatever. It allows you to provide an
//! asset of your choosing to users. You can configure how much they
//! receive per tap of the faucet, and you can limit how many times
//! per epoch the faucet can be tapped. You can also limit access to
//! tapping to only approved users (controlled via badges you can give
//! to people you approve of).
//!
//! Note that scrypto does not allow accurate timekeeping beyond the
//! rather crude counting of epochs so features like "taps per hour"
//! or "taps per day" are currently not feasible. More accurate time
//! period control is deferred until we have more information about
//! whether scrypto will eventually provide access to an accurate
//! clock.
//!
//! A future version of this faucet would support NFT taker badges to
//! enable more fine grained control of user access. For example, we
//! could then limit each user to a certain number of taps per time
//! period, different users could have different restrictions wrt how
//! much they receive in a tap, a badge could be rescinded, etc.
//!
//! A future version of this faucet would support itself minting the
//! tokens it hands out, as an alternative to being pool based.

use scrypto::prelude::*;

/// Helper function to make it easier to conditionally create an admin
/// badge the first time it's needed and then just reuse that one later.
/// It takes the current admin token as its input bucket, and if there is
/// something there just returns it unchanged. Otherwise it creates a new
/// bucket with an admin badge in it and returns that.
fn make_admin(badge_bucket: Option<Bucket>) -> Option<Bucket> {
    match badge_bucket {
        Some(_) => return badge_bucket,
        None => return Some(ResourceBuilder::new_fungible()
                            .divisibility(DIVISIBILITY_NONE)
                            .metadata("name", "Faucet admin badge")
                            .initial_supply(1)),
    }
}

/// Converts an option of bucket into an option of the resource address of
/// that bucket's asset
fn to_resource_addr(bucket: &Option<Bucket>) -> Option<ResourceAddress> {
    if let Some(bucket) = bucket {
        Some(bucket.resource_address())
    } else {
        None
    }
}


blueprint! {
    struct Faucet {
        funds: Vault,
        /// Quantity of token you receive per call to gimme
        tap_amount: Decimal,
        /// Number of calls to gimme we allow per epoch
        taps_per_epoch: Option<u64>,
        /// The last epoch someone called gimme on us
        last_active_epoch: u64,
        /// How many times gimme has been called in the current epoch
        taps_this_epoch: u64,
        /// You must have this to call any admin restricted methods
        admin_badge_addr: Option<ResourceAddress>,
        /// You must have this to call gimme if gimme has been restricted
        taker_badge_addr: Option<ResourceAddress>,
    }

    impl Faucet {

        /// Creates a new faucet.
        ///
        /// The following parameters are mandatory:
        ///
        /// funds - The faucet must start with funds, even if it has zero assets.
        ///         This is so we can know which asset type we're supposed to hold.
        ///
        /// tap_amount - This is the quantity of asset given to our users per call
        ///              to gimme.
        ///
        /// allow_empty_call - Set to true to allow admin to call the empty method,
        ///                    or set to false to allow nobody to call it.
        ///
        /// allow_stranger_fill - Set to true to allow anyone to add funds to the
        ///                       faucet, or set to false to only permit admin to
        ///                       do so.
        ///
        /// The following parameters are optional:
        ///
        /// taker_badge - Set this if you want to restrict gimme to be callable only
        ///               by people who have a badge. You need to supply the badge
        ///               yourself, e.g. by running the equivalent of
        ///               "resim new-badge-fixed 100" if you want 100 of the badges.
        ///
        /// taps_per_epoch - If you want to limit how fast people can run off with
        ///                  your tokens, set this and it controls how many calls
        ///                  to gimme can happen per epoch.
        ///
        /// The function returns a tuple containing the adddress of the new faucet
        /// and a bucket with the admin badge if one was created. Note that if you
        /// configure the faucet in such a way that there are no methods on it that
        /// require an admin badge, an admin badge will not be created.
        pub fn instantiate_faucet(
            taker_badge: Option<ResourceAddress>,
            funds: Bucket,
            tap_amount: Decimal,
            taps_per_epoch: Option<u64>,
            allow_empty_call: bool,
            allow_stranger_fill: bool) -> (ComponentAddress, Option<Bucket>)
        {
            assert!(taps_per_epoch.is_none() || taps_per_epoch.unwrap() > 0,
                    "Zero taps per epoch makes no sense, try with None or greater than zero.");

            assert!(tap_amount > 0.into(),
                    "Try with a tap_amount greater than zero.");

            let mut admin_badge: Option<Bucket> = None;


            let mut access_rules = AccessRules::new();

            if allow_empty_call {
                admin_badge = make_admin(admin_badge);
                access_rules = access_rules
                    .method("empty", rule!(require(admin_badge.as_ref().unwrap().resource_address())));
            } else {
                access_rules = access_rules
                    .method("empty", rule!(deny_all));
            }

            if !allow_stranger_fill {
                admin_badge = make_admin(admin_badge);
                access_rules = access_rules
                    .method("fill", rule!(require(admin_badge.as_ref().unwrap().resource_address())));
            }

            if let Some(taker_badge) = taker_badge {
                access_rules = access_rules
                    .method("gimme", rule!(require(taker_badge)));
            }
            access_rules = access_rules.default(rule!(allow_all));

            let faucet = Self {
                funds: Vault::with_bucket(funds),
                tap_amount,
                taps_per_epoch,
                last_active_epoch: Runtime::current_epoch(),
                taps_this_epoch: 0,
                admin_badge_addr: to_resource_addr(&admin_badge),
                taker_badge_addr: taker_badge,
            }
            .instantiate();
            (faucet.add_access_check(access_rules).globalize(), admin_badge)
        }

        /// Users should call this method to tap the faucet. If the faucet is
        /// empty it will panic. If it's run out for this epoch it will show
        /// an info message and return None. Otherwise it will return an amount
        /// of asset equal to the faucet's tap_amount.
        ///
        /// If a taker badge has been configured the user will need to prove
        /// possession of such a badge to call this method.
        pub fn gimme(&mut self) -> Option<Bucket> {
            if self.taps_per_epoch.is_some() {
                let epoch = Runtime::current_epoch();
                if epoch > self.last_active_epoch {
                    self.last_active_epoch = epoch;
                    self.taps_this_epoch = 1;
                } else {
                    if self.taps_this_epoch >= self.taps_per_epoch.unwrap() {
                        info!("The faucet is spent for this epoch, try again later!");
                        return None;
                    }
                    self.taps_this_epoch += 1;
                }
            }

            Some(self.funds.take(self.tap_amount))
        }

        /// This method adds funds to the faucet. All funds provided will be
        /// taken so long as they are of the correct asset type.
        ///
        /// If allow_stranger_fill has been set for the faucet anyone can
        /// call this method, otherwise you need to prove possession of an
        /// admin badge to do so.
        pub fn fill(&mut self, extra_funds: Bucket) {
            self.funds.put(extra_funds);
        }

        /// Empties out all the faucet's funds, returning them to the caller.
        ///
        /// Can only be called by someone with the admin badge, unless
        /// allow_empty_call is false in which case it cannot be called by anyone.
        pub fn empty(&mut self) -> Bucket {
            self.funds.take_all()
        }

        /// Retreives information about the faucet's current funding level.
        /// The tuple returned first has the asset resource address and then
        /// the amount of that asset left in the faucet.
        pub fn read_funding(&self) -> (ResourceAddress, Decimal) {
            (self.funds.resource_address(), self.funds.amount())
        }

        /// Tells you how many taps are possible in the current epoch.
        ///
        /// If the faucet has been configured to not have a per epoch
        /// limt this method panics.
        pub fn read_taps_remaining_epoch(&self) -> u64 {
            assert!(self.taps_per_epoch.is_some(),
                    "This faucet has no per epoch limit.");

            if Runtime::current_epoch() > self.last_active_epoch {
                self.taps_per_epoch.unwrap()
            } else {
                self.taps_per_epoch.unwrap() - self.taps_this_epoch
            }
        }

        /// Retrieves the faucet's static configuration. The returned tuple
        /// contains first the tap_amount and then the taps_per_epoch setting.
        pub fn read_config(&self) -> (Decimal, Option<u64>) {
            (self.tap_amount, self.taps_per_epoch)
        }

        /// Retrieves the faucet's admin badge address, if any.
        pub fn read_admin_badge_address(&self) -> Option<ResourceAddress> {
            self.admin_badge_addr
        }

        /// Retrieves the faucet's taker badge address, if any.
        pub fn read_taker_badge_address(&self) -> Option<ResourceAddress> {
            self.taker_badge_addr
        }
    }
}
