mod data;
use scrypto::prelude::*;
use data::*;

blueprint! {
	/// Implementation of LotteryEvent functionality that can be reused by anyone on the Radix ledger. 
	/// It generates the result numbers automatically and creates a distribution of the tokens in the pot based on the number of participants in each winning category. 
	/// Each participant receives a NFT containing the ticket data and can use this NFT to claim the prize once the event has ended. 
	/// 
	/// Properties:
	/// 
	/// * `current_pot`: This is the current amount of tokens that were gathered by selling tickets and will
	/// be split to the winners.
	/// * `auth_vault`: This is the vault that holds the badges.
	/// * `current_event`: This is the current event that is running.
	/// * `previous_events`: This is a list of previous events. When an event ends, the event is moved to
	/// this list. This is used for claiming prizes.
	/// * `registered_ticket_numbers`: This is a vector of vectors of ticket numbers. Each vector represents
	/// a ticket number. The outer vector is a list of all the ticket numbers. 
	/// We keep track for each registered ticket for the current event so that when the event ends we can find out if there are any winners
	/// * `lottery_ticket_address`: ResourceAddress,
	/// * `lottery_event_address`: ResourceAddress - this is the address of the current event.
	/// * `previous_event_pots` : HashMap with keys the NonFungibleId of the event and values the pot associated with that event
    struct Lottery {
        current_pot : Vault,
        auth_vault: Vault,
		current_event: Vault,
		previous_events: Vault,
		registered_ticket_numbers : Vec<Vec<u32>>,
		lottery_ticket_address: ResourceAddress,
		lottery_event_address: ResourceAddress,
		previous_event_pots : HashMap<NonFungibleId, Vault>
    }

    impl Lottery {
        pub fn instantiate(initial_pot : Bucket) -> ComponentAddress {
            let auth_token = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("name", "Admin authority for lottery manager")
				.burnable(rule!(allow_all), LOCKED)
                .initial_supply(1);
				
			let access_rules: AccessRules = AccessRules::new()
				.method("create_lottery_event", rule!(require(auth_token.resource_address())))
				.method("set_results", rule!(require(auth_token.resource_address())))
				.method("end_event_and_generate_random_results", rule!(require(auth_token.resource_address())))
				.method("clear_old_events", rule!(require(auth_token.resource_address())))
				.default(rule!(allow_all));

            // Create an NFT resource for lottery tickets with mutable supply
            let ticket_address = ResourceBuilder::new_non_fungible()
				.mintable(rule!(require(auth_token.resource_address())), LOCKED) // only the owner can mint new tickets
				.burnable(rule!(allow_all), LOCKED) // only the owner can mint new tickets
                .metadata("LotteryTicket", "Lottery Ticket with player numbers")
				.updateable_non_fungible_data(rule!(deny_all), LOCKED) // nobody can change the lottery numbers once created
                .no_initial_supply();
			
			// Create an NFT resource for lottery tickets with mutable supply
            let event_address = ResourceBuilder::new_non_fungible()
				.mintable(rule!(require(auth_token.resource_address())), LOCKED) // only the owner can mint new events
				.burnable(rule!(require(auth_token.resource_address())), LOCKED) // only the owner can burn previous events
                .metadata("LotteryEvent", "Lottery NFT containing the details")
				.updateable_non_fungible_data(rule!(require(auth_token.resource_address())), LOCKED) // only admin can change the lottery numbers once created
                .no_initial_supply();


            Self {
                current_pot: Vault::with_bucket(initial_pot),
				auth_vault: Vault::with_bucket(auth_token),
				current_event: Vault::new(event_address),
				previous_events: Vault::new(event_address),
				registered_ticket_numbers : Vec::<Vec::<u32>>::new(),
				lottery_ticket_address: ticket_address,
				lottery_event_address: event_address,
				previous_event_pots: HashMap::new()
            }
            .instantiate()
            .add_access_check(access_rules)
            .globalize()
        }

		fn get_random(end: u32) -> u32 {
			let num = Runtime::generate_uuid();
			(num % end as u128) as u32
		}

		fn generate_unique_random_numbers(number_of_winning_choices : u32, total_choices : u32) -> Vec<u32> {
			let mut winning_numbers : Vec<u32> = Vec::<u32>::new();

			// TODO instead of using this we should create a REST API request to some oracle like http://www.randomnumberapi.com/api/v1.0/random?min=1&max=49&count=6 for true random numbers
			for _ in 0..number_of_winning_choices {
				// loop until we generate a number that was not generated before
				let mut random_number : u32; 
				loop {
					random_number = blueprint::Lottery::get_random(total_choices as u32);
					let mut is_duplicate = false;
					for already_generated_number in winning_numbers.iter() {
						if *already_generated_number == random_number {
							is_duplicate = true;
							break;
						}
					}
							
					if !is_duplicate {
						break;
					}
				}
				winning_numbers.push(random_number);
			}

			return winning_numbers
		}

		fn compute_winner_token_distribution(&mut self, winners_count : Vec<u32>) -> (Bucket, Vec<Decimal>) {
			let nb_guesses_for_win : u32 = winners_count.len() as u32;
			let mut token_distribution : Vec<Decimal> = vec![Decimal::zero(); winners_count.len()];
			let base_percent : Decimal = Decimal::from(nb_guesses_for_win * (nb_guesses_for_win + 1) / 2); // compute the sum: (n * (n+1)) / 2
			let base_value : Decimal = self.current_pot.amount() / base_percent;


			let mut unclaimed_prize_bucket = Bucket::new(self.current_pot.resource_address());
			if winners_count.iter().sum::<u32>() == 0 {
				return (unclaimed_prize_bucket, token_distribution)  // no winners
			}
					
			// if no one guessed the highest amount of numbers, then the pot in those categories gets reported to the next event for an even bigger prize! 
			// We could distribute it to the others, but this would add more hype for future events
			if winners_count[winners_count.len() - 1] == 0 {
				// distribute the pot with the asumption that for each winning category we have 1 single person: 1 guess = 1 * base_value, 2 guesses = 2 * base_value and so on
				// if we have 4 winning choices where 1 is the lowest and 4 is the one with the highest number of guesses the token distribution in percent will be 10% 20% 30% 40%
				// if we have 9 winning choices, the distribution will be approximately 2.2% 4.4% 6.6% 8.8% 11% 13.2% 15.4% 17.6% 19.8% 
				let mut index_value : u32 = 0; 
				token_distribution.iter_mut().for_each(|value| {
					*value = base_value + Decimal::from(index_value) * base_value;
					index_value += 1;
				});

				let mut idx : i32 = nb_guesses_for_win as i32 - 1;
				while winners_count[idx as usize] == 0 && idx > 0 { 

					unclaimed_prize_bucket.put(self.current_pot.take(token_distribution[idx as usize]));
					token_distribution[idx as usize] = Decimal::zero();
					idx -= 1;
				}
			}

			// Using the distribution we computed above, if we have 2 people that guessed the max amount and only 1 person that guessed (max - 1) then the person that guessed less actually gets more tokes
			// We have to do a weighted distribution of the funds so that if there are more people that guessed 6 numbers they are guaranteed to receive a larger prize than the ones that guessed 5 numbers
			// To do this we solve a simple equation: if we have a * x + b * y + c * z = total_pot | where a,b,c are the number of winners in each category and x,y,z is the token distribution value that we calculated in the last step
			// then we know that y = 2 * z and x = 3 * z, substitute them by doing the sum below and find out the value of z by dividing the total pot. Once we found z, we can subtitute it back to find x and y
			let mut sum : u32 = 0;
			for idx in 0..(winners_count.len() - 1) {
				sum += winners_count[idx] * (idx as u32 + 1);
			}

			assert!(sum > 0, "sum should never be 0 here!");

			let weighted_base_value = self.current_pot.amount() / Decimal::from(sum);
			for idx in 0..(winners_count.len() - 1) {
				if winners_count[idx] > 0 {
					token_distribution[idx] = weighted_base_value * Decimal::from(idx + 1);
				}
				else {
					token_distribution[idx] = Decimal::zero();
				}
			}

			return (unclaimed_prize_bucket, token_distribution)
		}

		/// > Counts the number of winning tickets in each category based on the amount of guessed numbers 
		/// For each ticket, count the number of winning numbers in it, and increment the corresponding
		/// element in the `winners_count` vector
		/// 
		/// Arguments:
		/// 
		/// * `winning_numbers`: a vector of the winning numbers
		/// * `min_number_of_correct_picks_for_reward`: the minimum number of correct picks for a ticket to be
		/// considered a winner.
		/// 
		/// Returns:
		/// 
		/// A vector of u32
		fn compute_winners_count(&mut self, winning_numbers : Vec<u32>, min_number_of_correct_picks_for_reward : u32) -> Vec<u32> {
			let size : usize = winning_numbers.len() + 1 - min_number_of_correct_picks_for_reward as usize;
			let mut winners_count : Vec<u32> = vec![0 ; size];

        	for picked_ticket_numbers in self.registered_ticket_numbers.iter()
        	{
				let mut nb_winning_numbers_in_ticket : u32 = 0;
				for ticket_number in picked_ticket_numbers.iter() {
					if winning_numbers.contains(ticket_number) { 
						nb_winning_numbers_in_ticket += 1;
					}
				}
				
				if nb_winning_numbers_in_ticket >= min_number_of_correct_picks_for_reward {
					winners_count[(nb_winning_numbers_in_ticket - min_number_of_correct_picks_for_reward)  as usize] += 1;
				}
			}

			self.registered_ticket_numbers.clear();
			return winners_count
        }


		
		/// > This function generates random results for the event and ends it so that no tickets can be bought
		/// Also computes the tokens rewards for each category of winners based on the amount of guessed numbers
		pub fn end_event_and_generate_random_results(&mut self) {
			assert!(!self.current_event.is_empty(), "no event in progress");

			let lottery_event = self.current_event.non_fungible::<LotteryEventData>().data();


			let winning_numbers = blueprint::Lottery::generate_unique_random_numbers(lottery_event.number_of_winning_choices, lottery_event.event_choices.len() as u32);
			assert!(winning_numbers.len() == lottery_event.number_of_winning_choices as usize, "Lottery number number is correct");

			let winners_count = self.compute_winners_count(winning_numbers.clone(), lottery_event.min_number_of_correct_picks_for_reward);

			// this is where the magic gets done
			let (unclaimed_prize_bucket, token_distribution) = self.compute_winner_token_distribution(winners_count);

			self.previous_event_pots.entry(self.current_event.non_fungible::<LotteryEventData>().id())
			.or_insert(Vault::new(self.current_pot.resource_address()))
			.put(self.current_pot.take_all());

			if !unclaimed_prize_bucket.is_empty() {
				self.current_pot.put(unclaimed_prize_bucket);
			}
			let current_event = self.current_event.take_all();
			self.auth_vault
                .authorize(|| current_event.non_fungible().update_data(LotteryEventData {
					end_date : scrypto::prelude::Runtime::current_epoch(),
					winning_numbers : winning_numbers,
					winner_token_distribution : token_distribution,
					start_date : lottery_event.start_date,
					event_choices : lottery_event.event_choices.clone(), 
					number_of_winning_choices : lottery_event.number_of_winning_choices,
					min_number_of_correct_picks_for_reward : lottery_event.min_number_of_correct_picks_for_reward,
					ticket_price : lottery_event.ticket_price,
				}));

			self.previous_events.put(current_event);
		}

		/// > This function creates a new lottery event and stores it in the `current_event` vault
		/// 
		/// Arguments:
		/// 
		/// * `choices`: a vector of strings that represent the choices for the lottery event.
		/// * `num_winning_choices`: The number of choices that will be randomly selected as winners.
		/// * `min_nb_for_reward`: The minimum number of correct picks a player must have to be eligible for a
		/// reward.
		/// * `desired_ticket_price`: The price of a ticket in RADIX tokens.
		pub fn create_lottery_event(&mut self, choices : Vec<String>, num_winning_choices : u32, min_nb_for_reward : u32, desired_ticket_price : Decimal) 
		{
			// can be extended in the future to support simultaneous events but it's better to create different accounts if you want multiple events at the same time
			assert!(self.current_event.is_empty(), "cannot create a new event while one is already in progress");
			assert!(choices.len() >= 2, "minim size for event choices is 2");
			assert!(num_winning_choices < choices.len() as u32, "The number of winner choices cannot be than the total choices");
			assert!(min_nb_for_reward <= num_winning_choices as u32 && min_nb_for_reward >= 1, "The minimum number of winning choices cannot be larger than the total and less than 1");
			assert!(desired_ticket_price > Decimal::zero(), "The ticket price cannot be 0");

			let event_bucket = self.auth_vault.authorize(|| {
                borrow_resource_manager!(self.lottery_event_address)
                    .mint_non_fungible(&NonFungibleId::random(), 
					LotteryEventData {
						end_date : 0,
						winning_numbers : Vec::<u32>::new(),
						winner_token_distribution : Vec::<Decimal>::with_capacity((num_winning_choices - min_nb_for_reward).try_into().unwrap()),
						start_date : scrypto::prelude::Runtime::current_epoch(),
						event_choices : choices, 
						number_of_winning_choices : num_winning_choices,
						min_number_of_correct_picks_for_reward : min_nb_for_reward,
						ticket_price : desired_ticket_price,
					}
				)
            });

			self.current_event.put(event_bucket);
		}

		/// > The function buys a lottery ticket for the current event, and returns the ticket bucket and the
		/// remaining payment bucket
		/// 
		/// Arguments:
		/// 
		/// * `played_numbers`: The numbers that the player has chosen to play.
		/// * `payment`: The amount of tokens that the user is paying for the ticket.
        pub fn buy_lottery_ticket(&mut self, played_numbers : Vec<u32>, mut payment : Bucket ) -> (Bucket, Bucket) 
		{
            assert!(self.current_event.amount() == Decimal::one(), "no event in progress");
			let lottery_event : LotteryEventData = self.current_event.non_fungible::<LotteryEventData>().data();
			assert!(lottery_event.number_of_winning_choices == played_numbers.len() as u32, "Ticket choices count is different than the number of winning choices in the event");
			assert!(lottery_event.ticket_price <= payment.amount(), "Not enough tokens for purchasing a ticket for the lottery");
			assert!(*played_numbers.iter().max().unwrap() < lottery_event.event_choices.len() as u32, "played numbers exceed the maximum allowed for this event");
			assert!(payment.resource_address() == self.current_pot.resource_address(), "Payment tokens of this type are not accepted");

			self.current_pot.put(payment.take(lottery_event.ticket_price));
			
			self.registered_ticket_numbers.push(Vec::from(played_numbers.clone())); // used later to compute if there are any winners and how many
		
			let ticket_bucket = self.auth_vault.authorize(|| {
                borrow_resource_manager!(self.lottery_ticket_address)
                    .mint_non_fungible(&NonFungibleId::random(), 
					LotteryTicket  {
						event_id : self.current_event.non_fungible::<LotteryEventData>().id(),
						played_numbers : Vec::from(played_numbers.clone())
					})
            });


			return (ticket_bucket, payment)
        }

		/// The function takes a lottery ticket bucket as input, checks that the ticket is valid, and if it is,
		/// it returns a bucket containing the prize
		/// 
		/// Arguments:
		/// 
		/// * `ticket_bucket`: The bucket that contains the lottery ticket that the user wants to claim the
		/// prize for.
		/// 
		/// Returns:
		/// 
		/// The prize bucket
		pub fn claim_prize(&mut self, ticket_bucket : Bucket) -> Bucket {
			assert!(ticket_bucket.is_empty() || ticket_bucket.resource_address() == self.lottery_ticket_address, "This is not a valid lottery ticket");
			assert!(self.previous_events.amount() > Decimal::zero(), "There are no events open for claiming");
			let ticket: LotteryTicket = ticket_bucket.non_fungible().data();

			let event_bucket = self.previous_events.take_non_fungible(&ticket.event_id);
			assert!(!event_bucket.is_empty(), "There is no event associated with this ticket, either the event is still in progress or the claim period has expired and the event was deleted.");

			let event : LotteryEventData = event_bucket.non_fungible().data();

			let mut nb_guessed_numbers : u32 = 0;
			for number in ticket.played_numbers.iter() {
				if event.winning_numbers.contains(number) {
					nb_guessed_numbers += 1;
				}
			}

			assert!(nb_guessed_numbers >= event.min_number_of_correct_picks_for_reward, "Lottery ticket didn't guess enough numbers in order to qualify for reward");
		
			let winner_category = nb_guessed_numbers - event.min_number_of_correct_picks_for_reward;
			let prize : Decimal = event.winner_token_distribution[winner_category as usize];

			self.previous_events.put(event_bucket); // put the event bucket back in the previous events list 

			ticket_bucket.burn();

			return self.previous_event_pots.get_mut(&ticket.event_id).unwrap().take(prize) 
		}


		/// > This function clears all the events that have ended more than `nb_epochs_since_end_date` ago
		/// 
		/// Arguments:
		/// 
		/// * `nb_epochs_since_end_date`: the number of epochs since the end date of the event.
		pub fn clear_old_events(&mut self, nb_epochs_since_end_date : u64) { 
			// fail safe mechanism so that there is no scam possible where the admin of this component clears before users have time to claim the prize
			assert!(nb_epochs_since_end_date < 100, "cannot clear events older than 100 epochs"); 
			
			let events = self.previous_events.non_fungibles::<LotteryEventData>();
			for event in events {
				if event.data().end_date + nb_epochs_since_end_date < scrypto::prelude::Runtime::current_epoch() { 
					let event_id = &event.id();
					self.current_pot.put(self.previous_event_pots.get_mut(event_id).unwrap().take_all());
					self.previous_event_pots.remove(event_id);
					let event_bucket = self.previous_events.take_non_fungible(event_id);
					self.auth_vault.authorize(|| {
					event_bucket.burn();
					});
				}
			}
		}

		/// Function only intended for testing as we have access to private members and function. This should not be included in release version
		/// We create a lottery event, buy two tickets, end the event, and then claim the prize for the first
		/// ticket
		/// 
		/// Arguments:
		/// 
		/// * `funds_for_testing`: A bucket of tokens that will be used for testing.
		pub fn test_func(&mut self, mut funds_for_testing : Bucket)
		{
			assert!(self.current_event.is_empty(), "Cannot test while component has active events");
			ComponentAuthZone::push(self.auth_vault.create_proof());

			//*********************Test event creation***************************//
			let num_winning_choices = 5;
			let choices : Vec<_> = ["1","2","3","4","5","6","7","8","9"].iter().map(|s| s.to_string()).collect();
			self.create_lottery_event(choices, num_winning_choices, 1, Decimal::from(10 as u32));
			let event_id = self.current_event.non_fungible::<LotteryEventData>().id();

			assert!(!self.current_event.is_empty(), "created event");
			assert!(funds_for_testing.amount() > Decimal::from(50 as u32), "not enough");

			let initial_amount = self.current_pot.amount();

			
			//*********************Test that users can buy tickets and get the change back if they pay more***************************//
			let played_numbers : Vec<u32> = vec![0,1,2,3,4];

			let (lottery_ticket, ticket_change) = self.buy_lottery_ticket(played_numbers.clone(), funds_for_testing.take(Decimal::from(50 as u32)));
			let ticket_data : LotteryTicket = lottery_ticket.non_fungible().data();

			assert!(played_numbers.iter().zip(&ticket_data.played_numbers).filter(|&(a, b)| a == b).count() == played_numbers.len(), "Played numbers don't match");
			assert!(ticket_change.amount() == Decimal::from(40 as u32), "ticket change was not refunded");
			assert!(self.current_pot.amount() - initial_amount == Decimal::from(10 as u32), "Bought 3 tickets and got change");

			let played_numbers2 : Vec<u32> = vec![4,5,6,7,8];
			let (lottery_ticket2, ticket_change2) =  self.buy_lottery_ticket(played_numbers2, funds_for_testing.take(Decimal::from(10 as u32)));
			assert!(ticket_change2.amount() == Decimal::zero(), "no change");

			self.end_event_and_generate_random_results();

			//*********************Test that the event was ended correct, we have winners and that there are enough tokens in the event pot to distribute all the prize ***************************//
			assert!(self.current_event.is_empty(), "no event");
			assert!(!self.previous_events.is_empty(), "has previous event");
			assert!(self.previous_event_pots.contains_key(&event_id), "we have a previous event pot");

			let event_bucket = self.previous_events.take_non_fungible(&event_id);
			assert!(!event_bucket.is_empty(), "There is no event in the bucket");
			let lottery_event : LotteryEventData = event_bucket.non_fungible().data();

			self.previous_events.put(event_bucket); // put the event bucket back to its vault because we need to claim 
			
			assert!(lottery_event.number_of_winning_choices == num_winning_choices, "lottery event not updated correctly");
			assert!(lottery_event.winning_numbers.len() == lottery_event.number_of_winning_choices as usize, "lottery event winning numbers not computed");


			//*********************Test prize claiming***************************//
			let mut total_prize : Decimal = Decimal::zero();
			for prize in lottery_event.winner_token_distribution.iter() {
				total_prize += *prize;
			}
			assert!(total_prize > Decimal::zero() && total_prize <= self.previous_event_pots.get(&event_id).unwrap().amount(), "there was no winner");

			let prize = self.claim_prize(lottery_ticket);
			assert!(self.previous_event_pots.get(&event_id).unwrap().amount() >= total_prize - prize.amount(), "Funds not available for second ticket which is not claimed yet");

			lottery_ticket2.burn(); // unclaimed ticket
			self.current_pot.put(prize);
			self.current_pot.put(ticket_change);
			self.current_pot.put(ticket_change2);
			self.current_pot.put(funds_for_testing); // make sure we leave no bucket hanging in the air
			ComponentAuthZone::pop();
		}
    }
}