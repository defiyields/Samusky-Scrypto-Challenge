use sbor::*;
use scrypto::prelude::*;

/// A lottery ticket is a non-fungible token that represents a ticket for a
/// lottery event.
/// 
/// The event_id field is a NonFungibleId that represents the lottery event
/// that the ticket is for.
/// 
/// The played_numbers field is a vector of u32 values that represent the
/// numbers that the ticket holder played.
/// 
/// Properties:
/// 
/// * `event_id`: The ID of the lottery event.
/// * `played_numbers`: The numbers that the user has played.
#[derive(NonFungibleData)]
pub struct LotteryTicket {
    pub event_id : NonFungibleId,
    pub played_numbers : Vec<u32>
}

/// The `LotteryEventData` type represents a lottery event. It contains the end date, the winning
/// numbers, the winner token distribution, the start date, the event choices, the number of winning
/// choices, the minimum number of correct picks for a reward, and the ticket price.
/// 
/// Properties:
/// 
/// * `end_date`: the date when the lottery will end
/// * `winning_numbers`: the index of the choices that are correct/winning
/// * `winner_token_distribution`: This is a vector of Decimal values that represents the prize amount
/// for the winner of each category. For example, if the lottery has 6 winning numbers, then the last
/// value in this vector will be the prize amount for the winner who guessed all 6 numbers correctly.
/// * `start_date`: the date when the lottery starts
/// * `event_choices`: This is the list of choices that the users can pick from. For example, if the
/// lottery is a 6/49 lottery, then the choices will be the numbers from 1 to 49.
/// * `number_of_winning_choices`: the number of choices that will be deemed as correct from the list
/// (e.g. 6 winning numbers from a total of 49)
/// * `min_number_of_correct_picks_for_reward`: This is the minimum number of correct picks that a
/// ticket must have in order to be eligible for a reward.
/// * `ticket_price`: the price of a ticket
#[derive(NonFungibleData)]
pub struct LotteryEventData { 
    #[scrypto(mutable)]
    pub end_date : u64,
    #[scrypto(mutable)]
    pub winning_numbers : Vec<u32>,
    #[scrypto(mutable)]
    pub winner_token_distribution  : Vec<Decimal>,
    pub start_date : u64,
    pub event_choices : Vec<String>,
    pub number_of_winning_choices : u32,
    pub min_number_of_correct_picks_for_reward : u32,
    pub ticket_price : Decimal
}