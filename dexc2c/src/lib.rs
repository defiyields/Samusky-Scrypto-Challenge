use scrypto::prelude::*;

#[derive(TypeId, Encode, Decode, Describe, PartialEq, Clone)]
pub enum OrderStatus{
    Ask,
    Bid,
    Accepted,
    Escrowed,
    Paid,
    Released,
    Arbitration,
    Canceled,
    Withdrawed
}

#[derive(TypeId, Encode, Decode, Describe, Clone)]
pub struct OrderData {
    order_id: u64,
    buyer_ticket_id: NonFungibleId,
    seller_ticket_id: NonFungibleId,  
    // Token of the transaction
    token_address: ResourceAddress,
    // Number of transactions
    value: Decimal,
    // created time
    created_at_epoch: u64,
    last_update_epoch: u64,
    // Specify the operation window period (e.g. cancel)
    payment_window_in_epoch: u64,
    status: OrderStatus,
    // Customized Information
    desc: String,
    buyer_percent: u8,
    // Whether the buyer has withdrawn
    buyer_withdraw: bool,
    // Whether the seller has withdrawn
    seller_withdraw: bool,
}

#[derive(NonFungibleData)]
pub struct TicketData {
    #[scrypto(mutable)]
    pending_order_ids: Vec<u64>,
    deposit_amount: Decimal,
    // Amount required per transaction (deposit_amount/deposit_price = number of orders available for participation)
    deposit_price: Decimal,
    processed_order_cnt: Decimal,
    #[scrypto(mutable)]
    completed_order_ids: Vec<u64>
}

blueprint! {
    struct DexC2C {
        admin_badge_address: ResourceAddress,
        ticket_minter: Vault,
        ticket_resource_address: ResourceAddress,
        deposit_price: Decimal,
        // Assets held in escrow by sellers
        assets_vault_map: HashMap<ResourceAddress, Vault>,
        // Deposit for tickets Refundable after the transaction is completed
        deposit_vault: Vault,
        order_id_counter: u64,
        ticket_id_counter: u64,
        enabled: bool,
        payment_window_in_epoch: u64,
        // Supported token
        supported_token_list: Vec<ResourceAddress>,
        pending_order: HashMap<u64, OrderData>,
        history_order: HashMap<u64, OrderData>,
    }

    impl DexC2C {    
        pub fn new(deposit_price: Decimal) -> (ComponentAddress, Bucket){
            let admin_badge = ResourceBuilder::new_fungible()
            .divisibility(DIVISIBILITY_NONE)
            .metadata("name", "DexC2C Admin Badge")
            .initial_supply(1);

            let ticket_minter_badge = ResourceBuilder::new_fungible()
            .divisibility(DIVISIBILITY_NONE)
            .metadata("name", "Ticket Minter Badge")
            .initial_supply(1);

            let ticket_resource_address = ResourceBuilder::new_non_fungible()
            .metadata("name", "Trade Ticket Badge")
            .metadata("symbol", "TTT")
            .mintable(rule!(require(ticket_minter_badge.resource_address())), LOCKED)
            .updateable_non_fungible_data(rule!(require(ticket_minter_badge.resource_address())), LOCKED)
            .burnable(rule!(require(ticket_minter_badge.resource_address())), LOCKED)
            .no_initial_supply();

            let access_rules = AccessRules::new()
            .method("start_trade", rule!(require(admin_badge.resource_address())))
            .method("end_trade", rule!(require(admin_badge.resource_address())))
            .method("add_supported_token", rule!(require(admin_badge.resource_address())))
            .method("remove_supported_token", rule!(require(admin_badge.resource_address())))
            .method("resove_dispute", rule!(require(admin_badge.resource_address())))
            .default(rule!(allow_all));

            let component = Self {
                admin_badge_address: admin_badge.resource_address(),
                // order_minter: Vault::with_bucket(order_minter_badge),
                ticket_minter: Vault::with_bucket(ticket_minter_badge),
                // order_resource_address: order_resource_address,
                ticket_resource_address: ticket_resource_address,
                deposit_price,
                assets_vault_map: HashMap::new(),
                deposit_vault: Vault::new(RADIX_TOKEN),
                order_id_counter: 1,
                ticket_id_counter: 1,
                enabled: true,
                payment_window_in_epoch: 4,
                supported_token_list: vec![RADIX_TOKEN],
                pending_order: HashMap::new(),
                history_order: HashMap::new(),
            }.instantiate()
            .add_access_check(access_rules)
            .globalize();

            (component, admin_badge)
        }

        pub fn buy_ticket(&mut self, mut payment:Bucket) -> (Bucket, Bucket){
            assert!(self.enabled, "Component is not ready yet");
            assert!(payment.resource_address() == RADIX_TOKEN.into(), "You must use Radix (XRD).");
            
            let mut processed_order_cnt = (payment.amount() / self.deposit_price).floor();
            assert!(processed_order_cnt.ge(&Decimal::ONE), "Deposit is too small");
            if processed_order_cnt > Decimal::from("10") {
                processed_order_cnt = Decimal::from("10");
            }
            let deposit_amount = self.deposit_price * processed_order_cnt;

            self.deposit_vault.put(payment.take(deposit_amount));

            let data = TicketData{
                pending_order_ids: Vec::new(),
                deposit_price: self.deposit_price,
                processed_order_cnt: processed_order_cnt,
                completed_order_ids: Vec::new(),
                deposit_amount
            };
            
            let ticket = self.ticket_minter.authorize(||{
                borrow_resource_manager!(self.ticket_resource_address)
                .mint_non_fungible(&NonFungibleId::from_u64(self.ticket_id_counter), data)
            });
            self.ticket_id_counter += 1;

            return (payment, ticket);
        }

        // Buyer creates intention
        pub fn bid(&mut self, buyer: u64, seller: u64, token: ResourceAddress, value:Decimal, desc:String, auth: Bucket) -> (u64, Bucket) {
            assert!(self.enabled, "Component is not ready yet");
            assert!(auth.resource_address() == self.ticket_resource_address && auth.amount() == Decimal::ONE, "Invalid badge provided");
            assert!(self.supported_token_list.contains(&token), "Token not supported");
                        
            let ticket_id : NonFungibleId = auth.non_fungible::<TicketData>().id();
            assert!(ticket_id == NonFungibleId::from_u64(buyer), "Ticket is not available");

            // Verify that Tickets have exceeded the maximum number of orders that can be processed
            let mut ticket_data: TicketData = auth.non_fungible().data();
            assert!(Decimal::from(ticket_data.pending_order_ids.len()) <= ticket_data.processed_order_cnt, "Ticket is not available");
            
            let order_id = self.order_id_counter;
            let order_data = OrderData{
                order_id: order_id,
                buyer_ticket_id: NonFungibleId::from_u64(buyer),
                seller_ticket_id: NonFungibleId::from_u64(seller),
                token_address: token,
                value,
                created_at_epoch: Runtime::current_epoch(),
                last_update_epoch: Runtime::current_epoch(),
                payment_window_in_epoch: self.payment_window_in_epoch,
                status: OrderStatus::Bid,
                desc,
                buyer_percent: 100,
                buyer_withdraw: false,
                seller_withdraw: false,
            };

            self.order_id_counter += 1;

            ticket_data.pending_order_ids.push(order_id);
            self.ticket_minter.authorize(|| auth.non_fungible().update_data(ticket_data));

            self.pending_order.insert(order_id, order_data);            

            return (order_id, auth);
        }

        // Seller creates intention
        pub fn ask(&mut self, buyer: u64, seller: u64, token: ResourceAddress, value:Decimal, desc:String, auth: Bucket) -> (u64, Bucket) {
            assert!(self.enabled, "Component is not ready yet");
            assert!(auth.resource_address() == self.ticket_resource_address && auth.amount() == Decimal::ONE, "Invalid badge provided");
            assert!(self.supported_token_list.contains(&token), "Token not supported");

            let ticket_id : NonFungibleId = auth.non_fungible::<TicketData>().id();
            assert!(ticket_id == NonFungibleId::from_u64(seller), "Ticket is not available");

            // Verify that Tickets have exceeded the maximum number of orders that can be processed
            let mut ticket_data: TicketData = auth.non_fungible().data();
            assert!(Decimal::from(ticket_data.pending_order_ids.len()) <= ticket_data.processed_order_cnt, "Ticket is not available");
            
            let order_id = self.order_id_counter;
            let order_data = OrderData{
                order_id: order_id,
                buyer_ticket_id: NonFungibleId::from_u64(buyer),
                seller_ticket_id: NonFungibleId::from_u64(seller),
                token_address: token,
                value,
                created_at_epoch: Runtime::current_epoch(),
                last_update_epoch: Runtime::current_epoch(),
                payment_window_in_epoch: self.payment_window_in_epoch,
                status: OrderStatus::Ask,
                desc,
                buyer_percent: 100,
                buyer_withdraw: false,
                seller_withdraw: false,
            };

            self.order_id_counter += 1;

            ticket_data.pending_order_ids.push(order_id);
            self.ticket_minter.authorize(|| auth.non_fungible().update_data(ticket_data));

            self.pending_order.insert(order_id, order_data);            

            return (order_id, auth);
        }

        // Accept intention
        pub fn accept(&mut self, order_id:u64, auth:Bucket) -> Bucket{
            assert_eq!(auth.resource_address(), self.ticket_resource_address, "Invalid badge provided");
            assert_eq!(auth.amount(), dec!("1"), "Invalid badge provided");

            let ticket_data: TicketData = auth.non_fungible().data();
            assert!(Decimal::from(ticket_data.pending_order_ids.len()) <= ticket_data.processed_order_cnt, "Ticket is not available");

            let order_data = self.pending_order.get_mut(&order_id).unwrap();
            assert!(order_data.status == OrderStatus::Ask || order_data.status == OrderStatus::Bid, "Order status error");

            let ticket_res_mgr: &ResourceManager = borrow_resource_manager!(self.ticket_resource_address);
            let ticket_id : NonFungibleId = auth.non_fungible::<TicketData>().id();
            if order_data.status == OrderStatus::Ask {
                assert_eq!(ticket_id, order_data.buyer_ticket_id, "ASK order require buyer");
                let mut buyer_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&ticket_id);
                buyer_ticket_data.pending_order_ids.push(order_id);
                self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&ticket_id, buyer_ticket_data));
            } else {
                assert_eq!(ticket_id, order_data.seller_ticket_id, "BID order require seller");
                let mut seller_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&ticket_id);
                seller_ticket_data.pending_order_ids.push(order_id);
                self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&ticket_id, seller_ticket_data));
            }

            order_data.status = OrderStatus::Accepted;
            order_data.last_update_epoch = Runtime::current_epoch();
            
            return auth;
        }

        // Seller Escrow Funds
        pub fn seller_escrow(&mut self, order_id:u64, mut payment:Bucket, auth: Bucket) -> (Bucket, Bucket){
            assert_eq!(auth.resource_address(), self.ticket_resource_address, "Invalid badge provided");
            assert_eq!(auth.amount(), dec!("1"), "Invalid badge provided");

            let ticket_data: TicketData = auth.non_fungible().data();
            assert!(ticket_data.pending_order_ids.contains(&order_id), "Ticket is not available");
            assert!(self.pending_order.contains_key(&order_id), "Order do not exists");

            let order_data = self.pending_order.get_mut(&order_id).unwrap();
            assert!(order_data.status == OrderStatus::Accepted, "Order status error");

            let ticket_id : NonFungibleId = auth.non_fungible::<TicketData>().id();
            assert!(order_data.seller_ticket_id == ticket_id, "Must be Seller");
            assert!(order_data.token_address == payment.resource_address(), "Invalid payment token");
            assert!(payment.amount() >= order_data.value , "Payment not enough");
            
            if self.assets_vault_map.contains_key(&order_data.token_address) {
                let assert_vault = self.assets_vault_map.get_mut(&order_data.token_address).unwrap();
                assert_vault.put(payment.take(order_data.value));
            }else{
                let payment_vault = Vault::with_bucket(payment.take(order_data.value));
                self.assets_vault_map.insert(order_data.token_address, payment_vault);
            }
            
            order_data.status = OrderStatus::Escrowed;
            order_data.last_update_epoch = Runtime::current_epoch();

            return (payment, auth);
        }

        pub fn buyer_paid(&mut self, order_id:u64, auth:Bucket) -> Bucket {
            assert_eq!(auth.resource_address(), self.ticket_resource_address, "Invalid badge provided");
            assert_eq!(auth.amount(), dec!("1"), "Invalid badge provided");

            let ticket_data: TicketData = auth.non_fungible().data();
            assert!(ticket_data.pending_order_ids.contains(&order_id), "Ticket is not available");
            assert!(self.pending_order.contains_key(&order_id), "Order do not exists");

            let order_data = self.pending_order.get_mut(&order_id).unwrap();
            assert!(order_data.status == OrderStatus::Escrowed, "Order status error");
            
            let ticket_id : NonFungibleId = auth.non_fungible::<TicketData>().id();
            assert!(order_data.buyer_ticket_id == ticket_id, "Must be buyer");
            
            order_data.status = OrderStatus::Paid;
            order_data.last_update_epoch = Runtime::current_epoch();

            return auth;
        }

        pub fn seller_release(&mut self, order_id:u64, auth: Bucket) -> Bucket {
            assert_eq!(auth.resource_address(), self.ticket_resource_address, "Invalid badge provided");
            assert_eq!(auth.amount(), dec!("1"), "Invalid badge provided");

            let ticket_data: TicketData = auth.non_fungible().data();
            assert!(ticket_data.pending_order_ids.contains(&order_id), "Ticket is not available");
            assert!(self.pending_order.contains_key(&order_id), "Order do not exists");

            let order_data = self.pending_order.get_mut(&order_id).unwrap();
            assert!(order_data.status == OrderStatus::Paid, "Order status error");

            let ticket_id : NonFungibleId = auth.non_fungible::<TicketData>().id();
            assert!(order_data.seller_ticket_id == ticket_id, "Must be seller");
            
            order_data.status = OrderStatus::Released;
            order_data.last_update_epoch = Runtime::current_epoch();

            return auth;
        }

        pub fn buyer_withdraw(&mut self, order_id:u64, auth:Bucket) -> (Bucket, Bucket){
            assert_eq!(auth.resource_address(), self.ticket_resource_address, "Invalid badge provided");
            assert_eq!(auth.amount(), dec!("1"), "Invalid badge provided");

            let ticket_data: TicketData = auth.non_fungible().data();
            assert!(ticket_data.pending_order_ids.contains(&order_id), "Ticket is not available");
            assert!(self.pending_order.contains_key(&order_id), "Order do not exists");

            let order_data = self.pending_order.get_mut(&order_id).unwrap();
            assert!(order_data.status == OrderStatus::Released || order_data.status == OrderStatus::Arbitration, "Order status error");

            let ticket_id : NonFungibleId = auth.non_fungible::<TicketData>().id();
            assert!(order_data.buyer_ticket_id == ticket_id, "Must be buyer");

            
            let ticket_res_mgr: &ResourceManager = borrow_resource_manager!(self.ticket_resource_address);

            let asset_vault = self.assets_vault_map.get_mut(&order_data.token_address).unwrap();
            let val = order_data.value * (Decimal::from(order_data.buyer_percent) / Decimal::from(100));
            let withdraw = asset_vault.take(val);

            order_data.last_update_epoch = Runtime::current_epoch();
            order_data.buyer_withdraw = true;
            if order_data.buyer_percent == 100 {
                order_data.seller_withdraw = true;
                order_data.status = OrderStatus::Withdrawed;

                let mut buyer_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&order_data.buyer_ticket_id);
                buyer_ticket_data.pending_order_ids.retain(|item| item != &order_id);
                buyer_ticket_data.completed_order_ids.push(order_id);
                self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&order_data.buyer_ticket_id, buyer_ticket_data));

                let mut seller_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&order_data.seller_ticket_id);
                seller_ticket_data.pending_order_ids.retain(|item| item != &order_id);
                seller_ticket_data.completed_order_ids.push(order_id);
                self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&order_data.seller_ticket_id, seller_ticket_data));

                let history_order_data = order_data.clone();
                self.history_order.insert(order_id, history_order_data);

                self.pending_order.remove(&order_id);
            } else {
                if order_data.seller_withdraw { // If all buyers and sellers have withdrawn, update the order status to WITHDRAW
                    order_data.status = OrderStatus::Withdrawed;

                    let mut buyer_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&order_data.buyer_ticket_id);
                    buyer_ticket_data.pending_order_ids.retain(|item| item != &order_id);
                    buyer_ticket_data.completed_order_ids.push(order_id);
                    self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&order_data.buyer_ticket_id, buyer_ticket_data));
    
                    let mut seller_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&order_data.seller_ticket_id);
                    seller_ticket_data.pending_order_ids.retain(|item| item != &order_id);
                    seller_ticket_data.completed_order_ids.push(order_id);
                    self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&order_data.seller_ticket_id, seller_ticket_data));

                    let history_order_data = order_data.clone();
                    self.history_order.insert(order_id, history_order_data);

                    self.pending_order.remove(&order_id);
                }
            }

            return (withdraw, auth);
        }

        pub fn seller_withdraw(&mut self, order_id:u64, auth:Bucket) -> (Bucket, Bucket){
            assert_eq!(auth.resource_address(), self.ticket_resource_address, "Invalid badge provided");
            assert_eq!(auth.amount(), dec!("1"), "Invalid badge provided");

            let ticket_data: TicketData = auth.non_fungible().data();
            assert!(ticket_data.pending_order_ids.contains(&order_id), "Ticket is not available");
            assert!(self.pending_order.contains_key(&order_id), "Order do not exists");

            let order_data = self.pending_order.get_mut(&order_id).unwrap();

            assert!(order_data.status == OrderStatus::Arbitration, "Order status error");
            let ticket_id : NonFungibleId = auth.non_fungible::<TicketData>().id();
            assert!(order_data.seller_ticket_id == ticket_id, "Must be seller");

            
            let ticket_res_mgr: &ResourceManager = borrow_resource_manager!(self.ticket_resource_address);

            let asset_vault = self.assets_vault_map.get_mut(&order_data.token_address).unwrap();
            let val = order_data.value * (Decimal::from(1) - Decimal::from(order_data.buyer_percent) / Decimal::from(100));
            let withdraw = asset_vault.take(val);
        

            order_data.last_update_epoch = Runtime::current_epoch();
            order_data.seller_withdraw = true;
            if order_data.buyer_withdraw {
                order_data.status = OrderStatus::Withdrawed;
                let mut buyer_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&order_data.buyer_ticket_id);
                buyer_ticket_data.pending_order_ids.retain(|item| item != &order_id);
                buyer_ticket_data.completed_order_ids.push(order_id);
                self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&order_data.buyer_ticket_id, buyer_ticket_data));

                let mut seller_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&order_data.seller_ticket_id);
                seller_ticket_data.pending_order_ids.retain(|item| item != &order_id);
                seller_ticket_data.completed_order_ids.push(order_id);
                self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&order_data.seller_ticket_id, seller_ticket_data));

                let history_order_data = order_data.clone();
                self.history_order.insert(order_id, history_order_data);
            
                self.pending_order.remove(&order_id);
            }

            return (withdraw, auth);
        }

        // Resove dispute based on the agreement between the buyer and the seller, and determination of the buyer and seller ratio
        pub fn resove_dispute(&mut self, order_id: u64, buyer_percent:u8) {
            assert!(buyer_percent <= 100, "BuyerPercent must be 100 or lower");
            assert!(self.pending_order.contains_key(&order_id), "Order do not exists");
            let order_data = self.pending_order.get_mut(&order_id).unwrap();
            assert!(order_data.status == OrderStatus::Paid, "Order status error");

            order_data.status = OrderStatus::Arbitration;
            order_data.buyer_percent = buyer_percent;
            order_data.last_update_epoch = Runtime::current_epoch();
        }

        // No one accepts the intention, cancel the intention
        pub fn cancel(&mut self, order_id:u64, auth:Bucket) -> Bucket {
            assert_eq!(auth.resource_address(), self.ticket_resource_address, "Invalid badge provided");
            assert_eq!(auth.amount(), dec!("1"), "Invalid badge provided");

            let ticket_data: TicketData = auth.non_fungible().data();
            assert!(ticket_data.pending_order_ids.contains(&order_id), "Ticket is not available");
            assert!(self.pending_order.contains_key(&order_id), "Order do not exists");

            let ticket_id = auth.non_fungible::<TicketData>().id();
            let order_data = self.pending_order.get_mut(&order_id).unwrap();
            let ticket_res_mgr: &ResourceManager = borrow_resource_manager!(self.ticket_resource_address);

            assert!(order_data.status == OrderStatus::Ask || order_data.status == OrderStatus::Bid, "Order status error");
            if order_data.status == OrderStatus::Ask {
                assert!(order_data.seller_ticket_id == ticket_id, "Ticket is not available");
                let mut seller_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&order_data.seller_ticket_id);
                seller_ticket_data.pending_order_ids.retain(|item| item != &order_id);
                self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&order_data.seller_ticket_id, seller_ticket_data));
            }else {
                assert!(order_data.buyer_ticket_id == ticket_id, "Ticket is not available");
                let mut buyer_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&order_data.buyer_ticket_id);
                buyer_ticket_data.pending_order_ids.retain(|item| item != &order_id);
                self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&order_data.buyer_ticket_id, buyer_ticket_data));
            }

            return auth;
        }

        // If the buyer does not pay after the time, the seller cancels the order and returns the token
        pub fn seller_cancel(&mut self, order_id: u64, auth:Bucket) -> (Bucket, Bucket){
            assert_eq!(auth.resource_address(), self.ticket_resource_address, "Invalid badge provided");
            assert_eq!(auth.amount(), dec!("1"), "Invalid badge provided");

            let ticket_data: TicketData = auth.non_fungible().data();
            assert!(ticket_data.pending_order_ids.contains(&order_id), "Ticket is not available");
            assert!(self.pending_order.contains_key(&order_id), "Order do not exists");

            let order_data = self.pending_order.get_mut(&order_id).unwrap();
            assert!(order_data.status == OrderStatus::Escrowed, "Order status error");

            let ticket_id : NonFungibleId = auth.non_fungible::<TicketData>().id();
            assert!(order_data.seller_ticket_id == ticket_id, "Must be seller");

            assert!(Runtime::current_epoch() - order_data.last_update_epoch >= 4, "Not yet valid");

            order_data.status = OrderStatus::Canceled;

            let ticket_res_mgr: &ResourceManager = borrow_resource_manager!(self.ticket_resource_address);
            let mut buyer_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&order_data.buyer_ticket_id);
            buyer_ticket_data.pending_order_ids.retain(|item| item != &order_id);
            self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&order_data.buyer_ticket_id, buyer_ticket_data));

            let mut seller_ticket_data: TicketData = ticket_res_mgr.get_non_fungible_data(&order_data.seller_ticket_id);
            seller_ticket_data.pending_order_ids.retain(|item| item != &order_id);
            self.ticket_minter.authorize(|| ticket_res_mgr.update_non_fungible_data(&order_data.seller_ticket_id, seller_ticket_data));

            let assert_vault = self.assets_vault_map.get_mut(&order_data.token_address).unwrap();
            let ret_bucket = assert_vault.take(order_data.value);

            let history_order_data = order_data.clone();
            self.history_order.insert(order_id, history_order_data);

            self.pending_order.remove(&order_id);

            return (ret_bucket, auth);
        }

        pub fn refund_ticket(&mut self, auth: Bucket) -> Bucket{
            assert_eq!(auth.resource_address(), self.ticket_resource_address, "Invalid badge provided");
            assert_eq!(auth.amount(), dec!("1"), "Invalid badge provided");

            let ticket_data: TicketData = auth.non_fungible().data();
            assert!(ticket_data.pending_order_ids.is_empty(), "Ticket Pending Orders");
            
            let refund_bucket = self.deposit_vault.take(ticket_data.deposit_amount);

            let ticket_res_mgr: &ResourceManager = borrow_resource_manager!(self.ticket_resource_address);
            self.ticket_minter.authorize(|| ticket_res_mgr.burn(auth));

            return refund_bucket;
        }
        
        pub fn get_pending_order_by_id(&self, order_id:u64) -> OrderData{
            assert!(self.pending_order.contains_key(&order_id), "Order do not exists");

            return self.pending_order.get(&order_id).unwrap().clone();
        }

        pub fn get_history_order_by_id(&self, order_id:u64) -> OrderData{
            assert!(self.history_order.contains_key(&order_id), "Order do not exists");

            return self.history_order.get(&order_id).unwrap().clone();
        }

        pub fn get_pending_orders(&self) -> HashMap<u64,OrderData>{
            return self.pending_order.clone();
        }

        pub fn get_history_order(&self) -> HashMap<u64,OrderData>{
            return self.history_order.clone();
        }

        pub fn start_trade(&mut self) {
            self.enabled = true;
        }

        pub fn close_trade(&mut self){
            self.enabled = false;
        }

        pub fn get_deposit_price(&self) -> Decimal{
            return self.deposit_price;
        }

        pub fn update_deposit_price(&mut self, price: Decimal){
            assert!(price > Decimal::ZERO, "Price cannot be zero");
            self.deposit_price = price;
        }

        pub fn get_supported_token(&self) -> Vec<ResourceAddress>{
            return self.supported_token_list.to_vec();
        }

        pub fn add_supported_token(&mut self, token_resource_address: ResourceAddress){
            assert!(!self.supported_token_list.contains(&token_resource_address), "Token already exists");
            self.supported_token_list.push(token_resource_address);
        }

        pub fn remove_supported_token(&mut self, token_resource_address: ResourceAddress){
            assert!(self.supported_token_list.contains(&token_resource_address), "Token do not exists");
            self.supported_token_list.retain(|item| if item.eq(&token_resource_address) {false} else {true});
        }
        
    }
}