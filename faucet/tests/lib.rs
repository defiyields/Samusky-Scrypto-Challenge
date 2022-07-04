//! NOTE these tests use a global resource (the resim exectuable's simulator) and
//! therefore MUST be run single threaded, like this from the command line:
//! cargo test -- --test-threads=1

use std::process::Command;
use regex::Regex;
use lazy_static::lazy_static;

const RADIX_TOKEN: &str = "030000000000000000000000000000000000000000000000000004";

#[derive(Debug)]
struct Account {
    address: String,
    _pubkey: String,
    privkey: String,
}

#[derive(Debug)]
struct FaucetComponent {
    address: String,
    _blueprint_addr: String,
    admin_badge_addr: Option<String>,
}

/// Runds a command line program, panicking if it fails and returning its
/// stdout if it succeeds
fn run_command(command: &mut Command) -> String {
    let output = command
        .output()
        .expect("Failed to run command line");
    assert!(output.status.success(),
            "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Calls "resim reset"
fn reset_sim() {
    run_command(Command::new("resim")
        .arg("reset"));
}

/// Calls "resim new-account"
///
/// Returns a tuple containing first the new account's address, then
/// its public key, and then last its private key.
fn create_account() -> Account {
    let output = run_command(Command::new("resim")
                             .arg("new-account"));

    lazy_static! {
        static ref RE_ADDRESS: Regex = Regex::new(r"Account component address: (\w*)").unwrap();
        static ref RE_PUBKEY:  Regex = Regex::new(r"Public key: (\w*)").unwrap();
        static ref RE_PRIVKEY: Regex = Regex::new(r"Private key: (\w*)").unwrap();
    }
    let address = &RE_ADDRESS.captures(&output).expect("Failed to parse new-account address")[1];
    let pubkey = &RE_PUBKEY.captures(&output).expect("Failed to parse new-account pubkey")[1];
    let privkey = &RE_PRIVKEY.captures(&output).expect("Failed to parse new-account privkey")[1];

    Account {
        address: address.to_string(),
        _pubkey: pubkey.to_string(),
        privkey: privkey.to_string()
    }
}

/// Publishes the faucet blueprint by calling "resim publish ."
///
/// Returns the new blueprint's address
fn publish_faucet_component() -> String {
    let output = run_command(Command::new("resim")
                             .arg("publish")
                             .arg("."));
    lazy_static! {
        static ref RE_ADDRESS: Regex = Regex::new(r"New Package: (\w*)").unwrap();
    }
    
    RE_ADDRESS.captures(&output).expect("Failed to parse new blueprint address")[1].to_string()
}

/// Creates a new faucet instance by calling "resim run tests/instantiate_faucet.tm ..."
///
/// Returns the faucet created.
fn instantiate_faucet(account_addr: &str, blueprint_addr: &str,
                      taker_badge: Option<&str>,
                      asset_addr: &str, asset_amount: &str,
                      tap_amount: &str, taps_per_epoch: &str,
                      allow_empty_call: bool, allow_stranger_fill: bool) -> FaucetComponent {
    let output = run_command(Command::new("resim")
                             .arg("run")
                             .arg("tests/instantiate_faucet.tm")
                             .env("account", account_addr)
                             .env("package", &blueprint_addr)
                             .env("taker_badge", &option_to_tm_string(taker_badge))
                             .env("asset", &asset_addr)
                             .env("amount", &asset_amount)
                             .env("tap_amount", &tap_amount)
                             .env("taps_per_epoch", &taps_per_epoch)
                             .env("allow_empty_call", allow_empty_call.to_string())
                             .env("allow_stranger_fill", allow_stranger_fill.to_string()));
    lazy_static! {
        static ref RE_ADDRESS: Regex = Regex::new(r#"├─ Tuple\(ComponentAddress\("(\w*)"\)"#).unwrap();
        static ref RE_BADGE: Regex = Regex::new(r#"└─ Resource: (\w*)"#).unwrap();
    }

    let admin_badge_addr = if let Some(hit) = RE_BADGE.captures(&output) {
        Some(hit[1].to_string())
    } else {
        None
    };
    
    FaucetComponent {
        address: RE_ADDRESS.captures(&output).expect("Failed to parse new faucet address")[1].to_string(),
        _blueprint_addr: blueprint_addr.to_string(),
        admin_badge_addr
    }
}

/// Changes the default account by calling "resim set-default-account ..."
fn set_default_account(account: &Account) {
    run_command(Command::new("resim")
                .arg("set-default-account")
                .arg(&account.address)
                .arg(&account.privkey));
}

/// Retreives a user's current balance for the requested asset by calling "resim show ..."
fn get_balance(account: &Account, resource_addr: &str) -> String {
    let output = run_command(Command::new("resim")
                             .arg("show")
                             .arg(&account.address));
    let regexp = r#".─ \{ amount: (\d*), resource address: "#.to_string() + resource_addr + ",";
    let re_balance: Regex = Regex::new(&regexp).unwrap();

    re_balance.captures(&output).expect("Failed to parse balance")[1].to_string()
}

/// Retrieves the remaining balance in a faucet by calling "resim show ..."
fn get_faucet_balance(faucet: &FaucetComponent) -> String {
    let output = run_command(Command::new("resim")
                             .arg("show")
                             .arg(&faucet.address));
    lazy_static! {
        static ref RE_BALANCE: Regex = Regex::new(r#".─ \{ amount: (\d*), resource address: "#).unwrap();
    }

    RE_BALANCE.captures(&output).expect("Failed to parse faucet balance")[1].to_string()
}

/// Calls the faucet, returning the number of taps still allowed in this epoch. Note
/// that this will panic if you call it on a faucet where this has been set to None.
fn call_read_taps_remaining_epoch(faucet: &FaucetComponent) -> String {
    let output = run_command(Command::new("resim")
                             .arg("call-method")
                             .arg(&faucet.address)
                             .arg("read_taps_remaining_epoch"));
    lazy_static! {
        static ref RE_BALANCE: Regex = Regex::new(r#"Instruction Outputs:\n.─ (\d*)u64"#).unwrap();
    }

    RE_BALANCE.captures(&output).expect("Failed to parse faucet balance")[1].to_string()
}

/// Calls gimme on the faucet, without providing any badge proof. If successful this will
/// result in a transfer of funds from the faucet to the user.
fn call_gimme(faucet: &FaucetComponent) {
    run_command(Command::new("resim")
                .arg("call-method")
                .arg(&faucet.address)
                .arg("gimme"));
}

/// Calls gimme on the faucet, using the provided badge as proof. If successful this will
/// result in a transfer of funds from the faucet to the user.
fn call_gimme_with_badge(faucet: &FaucetComponent, account: &Account, badge_addr: &str) {
    run_command(Command::new("resim")
                .arg("run")
                .arg("tests/call_gimme.tm")
                .env("account", &account.address)
                .env("component", &faucet.address)
                .env("taker_badge", &badge_addr)
                );
}

/// Calls empty on the faucet, without providing any badge proof. If successful the faucet
/// will be empty and the user will receive all its funds.
fn call_empty_no_badge(faucet: &FaucetComponent) {
    run_command(Command::new("resim")
                .arg("call-method")
                .arg(&faucet.address)
                .arg("empty"));
}

/// Calls empty on the faucet, using the provided badge as proof. If successful the faucet
/// will be empty and the user will receive all its funds.
fn call_empty(faucet: &FaucetComponent, account: &Account, badge: Option<&str>) {
    let badge = if let Some(x) = badge { x } else { &faucet.admin_badge_addr.as_ref().unwrap() };
    run_command(Command::new("resim")
                .arg("run")
                .arg("tests/call_empty.tm")
                .env("account", &account.address)
                .env("component", &faucet.address)
                .env("admin_badge", &badge)
                );
}

/// Calls fill on the faucet, without providing any badge proof. If successful funds will
/// be transferred from the user to the faucet.
fn call_fill(faucet: &FaucetComponent, account: &Account, resource_addr: &str, amount: &str) {
    run_command(Command::new("resim")
                .arg("run")
                .arg("tests/call_fill.tm")
                .env("account", &account.address)
                .env("component", &faucet.address)
                .env("amount", &amount)
                .env("resource", &resource_addr)
                );
}

/// Calls fill on the faucet, using the provided badge as proof. If successful funds will
/// be transferred from the user to the faucet.
fn call_fill_with_badge(faucet: &FaucetComponent, account: &Account,
                        admin_badge: &str,
                        resource_addr: &str, amount: &str) {
    run_command(Command::new("resim")
                .arg("run")
                .arg("tests/call_fill_with_badge.tm")
                .env("account", &account.address)
                .env("component", &faucet.address)
                .env("admin_badge", &admin_badge)
                .env("amount", &amount)
                .env("resource", &resource_addr)
                );
}

/// Retrieves the remaining funding levels of the faucet.
/// Return tuple is first asset resource address, then amount of that asset
fn call_read_funding(faucet: &FaucetComponent) -> (String, String) {
    let output = run_command(Command::new("resim")
                             .arg("call-method")
                             .arg(&faucet.address)
                             .arg("read_funding"));
    lazy_static! {
        static ref RE_FUNDING: Regex = Regex::new(r#"Instruction Outputs:\n.─ Tuple\(ResourceAddress\("(.*)"\), Decimal\("(.*)""#).unwrap();
    }

    let results = RE_FUNDING.captures(&output).expect("Failed to parse faucet funding");
    (results[1].to_string(), results[2].to_string())
}

/// Retrieves the static configuration of the faucet.
/// Return tuple is first amount per tap then Option of taps per epoch
fn call_read_config(faucet: &FaucetComponent) -> (String, Option<String>) {
    let output = run_command(Command::new("resim")
                             .arg("call-method")
                             .arg(&faucet.address)
                             .arg("read_config"));
    lazy_static! {
        static ref RE_CONFIG: Regex = Regex::new(r#"Instruction Outputs:\n.─ Tuple\(Decimal\("(.*)"\), (.*)\)"#).unwrap();
    }

    let results = RE_CONFIG.captures(&output).expect("Failed to parse faucet config");
    (results[1].to_string(), maybe_some(&results[2]))
}

/// Retrieves the address used by the faucet's admin badge if any. Note that if the faucet
/// doesn't have an admin badge this function will panic.
fn call_read_admin_badge_address(faucet: &FaucetComponent) -> String {
    let output = run_command(Command::new("resim")
                             .arg("call-method")
                             .arg(&faucet.address)
                             .arg("read_admin_badge_address"));
    lazy_static! {
        static ref RE_ADMIN_BADGE: Regex = Regex::new(r#"Instruction Outputs:\n.─ Some\(ResourceAddress\("(.*)""#).unwrap();
    }

    RE_ADMIN_BADGE.captures(&output).expect("Failed to parse faucet admin badge address")[1].to_string()
}

/// Retrieves the address used by the faucet's taker badge if any. Note that if the faucet
/// doesn't have a taker badge this function will panic.
fn call_read_taker_badge_address(faucet: &FaucetComponent) -> String {
    let output = run_command(Command::new("resim")
                             .arg("call-method")
                             .arg(&faucet.address)
                             .arg("read_taker_badge_address"));
    lazy_static! {
        static ref RE_TAKER_BADGE: Regex = Regex::new(r#"Instruction Outputs:\n.─ Some\(ResourceAddress\("(.*)""#).unwrap();
    }

    RE_TAKER_BADGE.captures(&output).expect("Failed to parse faucet taker badge address")[1].to_string()
}

/// Given a string of the form "None" or "Some(string)" returns either
/// a None or a Some(string)
fn maybe_some(input: &str) -> Option<String> {
    if input == "None" {
        return None;
    }
    lazy_static! {
        static ref RE_OPTION: Regex = Regex::new(r#"Some\((.*)\)"#).unwrap();
    }
    Some(RE_OPTION.captures(&input).expect("Invalid string-form Option")[1].to_string())
}

/// Converts an Option<&str> where the str is a resource address into a
/// string that can be used inside a transaction manifest. For example,
/// None -> the string None
/// Some(03000...04) -> the string Some(ResourceAddress("03000...04"))
fn option_to_tm_string(input: Option<&str>) -> String {
    if input.is_none() { "None".to_string() } else { "Some(ResourceAddress(\"".to_string() + input.unwrap() + "\"))" }
}

/// Calls "resim set-current-epoch ..." to change the epoch
fn set_current_epoch(epoch: u64) {
    run_command(Command::new("resim")
                .arg("set-current-epoch")
                .arg(epoch.to_string())
                );
}

/// Calls "resim new-badge-fixed ..." to create a new badge type.
/// Returns the resource address of the new badge.
fn new_badge_fixed(name: &str, symbol: &str, supply: &str) -> String {
    let output = run_command(Command::new("resim")
                             .arg("new-badge-fixed")
                             .arg("--name")
                             .arg(&name)
                             .arg("--symbol")
                             .arg(&symbol)
                             .arg(&supply));
    lazy_static! {
        static ref RE_BADGE_ADDR: Regex = Regex::new(r#"Instruction Outputs:\n.─ Tuple\(ResourceAddress\("(.*)""#).unwrap();
    }

    RE_BADGE_ADDR.captures(&output).expect("Failed to parse new badge address")[1].to_string()
}

/// Calls "resim new-token-fixed ..." to create a new token.
/// Returns the resource address of the new token.
fn new_token_fixed(name: &str, symbol: &str, supply: &str) -> String {
    let output = run_command(Command::new("resim")
                             .arg("new-token-fixed")
                             .arg("--name")
                             .arg(&name)
                             .arg("--symbol")
                             .arg(&symbol)
                             .arg(&supply));
    lazy_static! {
        static ref RE_TOKEN_ADDR: Regex = Regex::new(r#"Instruction Outputs:\n.─ Tuple\(ResourceAddress\("(.*)""#).unwrap();
    }

    RE_TOKEN_ADDR.captures(&output).expect("Failed to parse new token address")[1].to_string()
}

/// Calls "resim transfer ..." to transfer tokens from the default account to another.
fn transfer_tokens(to: &Account, asset: &str, amount: &str) {
    run_command(Command::new("resim")
                .arg("transfer")
                .arg(&amount)
                .arg(&asset)
                .arg(&to.address));
}


#[test]
fn test_init_negative_tap_amount() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();
    let alice = create_account();
    let result = std::panic::catch_unwind(||
                                          instantiate_faucet(&alice.address, &blueprint_addr,
                                                             None,
                                                             RADIX_TOKEN, "100",
                                                             "-1", "None",
                                                             true, true));
    assert!(result.is_err(),
            "Negative tap_amount should be panic");
}

#[test]
fn test_init_zero_tap_amount() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();
    let alice = create_account();
    let result = std::panic::catch_unwind(||
                                          instantiate_faucet(&alice.address, &blueprint_addr,
                                                             None,
                                                             RADIX_TOKEN, "100",
                                                             "0", "None",
                                                             true, true));
    assert!(result.is_err(),
            "Zero tap_amount should be panic");
}

#[test]
fn test_init_negative_taps_per_epoch() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();
    let alice = create_account();
    let result = std::panic::catch_unwind(||
                                          instantiate_faucet(&alice.address, &blueprint_addr,
                                                             None,
                                                             RADIX_TOKEN, "100",
                                                             "1", "Some(-1)",
                                                             true, true));
    assert!(result.is_err(),
            "Negative taps per epoch should be panic");
}

#[test]
fn test_init_zero_taps_per_epoch() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();
    let alice = create_account();
    let result = std::panic::catch_unwind(||
                                          instantiate_faucet(&alice.address, &blueprint_addr,
                                                             None,
                                                             RADIX_TOKEN, "100",
                                                             "1", "Some(0)",
                                                             true, true));
    assert!(result.is_err(),
            "Zero taps per epoch should be panic");
}

#[test]
fn test_tap_many_times_when_no_epoch_limit() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();
    let alice = create_account();
    let faucet = instantiate_faucet(&alice.address, &blueprint_addr,
                                    None,
                                    RADIX_TOKEN, "100",
                                    "1", "None",
                                    false, true);
    
    assert_eq!(get_balance(&alice, RADIX_TOKEN), "999900",
               "Alice should have spent 100 XRD so far");
    for _ in 0..50 {
        call_gimme(&faucet);
    }
    assert_eq!(get_balance(&alice, RADIX_TOKEN), "999950",
               "Alice should have half her money back");
}

#[test]
fn test_no_admin_badge_when_not_needed() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();
    let alice = create_account();
    let faucet = instantiate_faucet(&alice.address, &blueprint_addr,
                                    None,
                                    RADIX_TOKEN, "100",
                                    "1", "None",
                                    false, true);
    let result = std::panic::catch_unwind(||
                                          call_read_admin_badge_address(&faucet));
    assert!(result.is_err(),
            "There shouldn't be an admin badge");
}

#[test]
fn test_has_admin_badge_when_empty_allowed() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();
    let alice = create_account();
    let faucet = instantiate_faucet(&alice.address, &blueprint_addr,
                                    None,
                                    RADIX_TOKEN, "100",
                                    "1", "None",
                                    true, true);
    assert_eq!(faucet.admin_badge_addr.as_ref().unwrap(), &call_read_admin_badge_address(&faucet),
               "This faucet should report having an admin badge");
}

#[test]
fn test_has_admin_badge_when_no_stranger_fill() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();
    let alice = create_account();
    let faucet = instantiate_faucet(&alice.address, &blueprint_addr,
                                    None,
                                    RADIX_TOKEN, "100",
                                    "1", "None",
                                    false, false);
    assert_eq!(faucet.admin_badge_addr.as_ref().unwrap(), &call_read_admin_badge_address(&faucet),
               "This faucet should report having an admin badge");
}

#[test]
fn test_call_empty_when_disallowed() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();
    let alice = create_account();
    let faucet = instantiate_faucet(&alice.address, &blueprint_addr,
                                    None,
                                    RADIX_TOKEN, "100",
                                    "1", "None",
                                    false, false);
    let result = std::panic::catch_unwind(||
                                          call_empty_no_badge(&faucet));
    assert!(result.is_err(),
            "Should not be able to call empty when empty is not allowed");
    let result = std::panic::catch_unwind(||
                                          call_empty(&faucet, &alice, None));
    assert!(result.is_err(),
            "Should not be able to call empty when empty is not allowed, even with admin badge");
}

#[test]
fn test_read_taps_remaining_when_not_set() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();
    let alice = create_account();
    let faucet = instantiate_faucet(&alice.address, &blueprint_addr,
                                    None,
                                    RADIX_TOKEN, "100",
                                    "7", "None",
                                    false, false);
    let result = std::panic::catch_unwind(||
                                          call_read_taps_remaining_epoch(&faucet));
    assert!(result.is_err(),
            "Cannot read taps remaining when not set");
    let result = call_read_config(&faucet);
    assert_eq!("7", result.0, "Tap amount should be as configured");
    assert!(result.1.is_none(), "Tap per epoch should be as configured");
}

/// Alice creates a faucet without taker badges and puts it through its paces
#[test]
fn test_scenario_1() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();

    // Alice owns the faucet
    let alice = create_account();
    let faucet = instantiate_faucet(&alice.address, &blueprint_addr,
                                    None,
                                    RADIX_TOKEN, "100",
                                    "1", "Some(10u64)",
                                    true, true);

    assert_eq!("100", get_faucet_balance(&faucet),
              "Faucet should start with 100 tokens");

    // Bob is a faucet user
    let bob = create_account();
    set_default_account(&bob);
    assert_eq!(get_balance(&bob, RADIX_TOKEN), "1000000",
               "Bob should start with a cool mill");

    // Bob uses the faucet
    call_gimme(&faucet);
    assert_eq!(get_balance(&bob, RADIX_TOKEN), "1000001",
               "After tapping the faucet Bob should have a whole extra XRD");
    assert_eq!("99", get_faucet_balance(&faucet),
              "Faucet should have 99 left after a tap");

    // Bob tries to steal the faucet's funds by calling empty()
    // First without presenting a badge
    let result = std::panic::catch_unwind(||
                                          call_empty_no_badge(&faucet));
    assert!(result.is_err(),
            "Bob trying to steal funds should panic due to access violation");
    // Then in the mistaken hope he has a badge to use
    let result = std::panic::catch_unwind(||
                                          call_empty(&faucet, &bob, None));
    assert!(result.is_err(),
            "Bob shouldn't have a badge to present");

    // Alice (suspecting foul play from Bob perhaps) empties the faucet
    set_default_account(&alice);
    call_empty(&faucet, &alice, None);
    assert_eq!(get_balance(&alice, RADIX_TOKEN), "999999",
               "After emptying remaining funds Alice should have 999999 XRD");
    assert_eq!("0", get_faucet_balance(&faucet),
              "Faucet should be empty after being emptied");

    // Emptying an empty faucet should still succeed
    call_empty(&faucet, &alice, None);
    assert_eq!(get_balance(&alice, RADIX_TOKEN), "999999",
               "After emptying zero remaining funds Alice should still have 999999 XRD");
    assert_eq!("0", get_faucet_balance(&faucet),
              "Faucet should be still empty after being emptied twice");

    // Bob, regretful, tries to make amends by adding some funds
    set_default_account(&bob);
    call_fill(&faucet, &bob, RADIX_TOKEN, "50");
    assert_eq!(get_balance(&bob, RADIX_TOKEN), "999951",
               "After refilling the faucet Bob should have 50 XRD less");
    assert_eq!("50", get_faucet_balance(&faucet),
              "Refilled faucet should be in the money again");

    // A relieved Alice also gets back in the game
    set_default_account(&alice);
    call_fill(&faucet, &alice, RADIX_TOKEN, "99");
    assert_eq!(get_balance(&alice, RADIX_TOKEN), "999900",
               "Alice should be back down again after filling the faucet");
    assert_eq!("149", get_faucet_balance(&faucet),
              "Twice refilled faucet should be fuller than ever");
    
    // After Bob's intial tap there should only be 9 left in this epoch
    // Let's see if it will fail on the 11th and also check the read
    // function during
    set_default_account(&bob);
    for attempt in 1..10 {
        assert_eq!((10-attempt).to_string(), call_read_taps_remaining_epoch(&faucet),
                   "Should only be {} left for this epoch before a tap", (10-attempt));
        call_gimme(&faucet);
        assert_eq!((9-attempt).to_string(), call_read_taps_remaining_epoch(&faucet),
                   "Should only be {} left for this epoch after a tap", (9-attempt));
    }
    // Now there's none left this epoch and it should fail but not panic
    call_gimme(&faucet);
    assert_eq!("0", call_read_taps_remaining_epoch(&faucet),
               "Faucet should be spent for this epoch");
    assert_eq!(get_balance(&bob, RADIX_TOKEN), "999960",
               "After tapping the whole epoch Bob should have another 9 XRD");

    // A new epoch dawns and Bob should be able to tap again
    set_current_epoch(5);
    assert_eq!("10", call_read_taps_remaining_epoch(&faucet),
               "New epoch should have new taps available");

    call_gimme(&faucet);
    assert_eq!("9", call_read_taps_remaining_epoch(&faucet),
               "New epoch should be down one tap");
    assert_eq!(get_balance(&bob, RADIX_TOKEN), "999961",
               "Bob should have one more XRD after the new epoch's tap");

    // Even if we increase epochs again, we don't get the epoch
    // max added to current available taps, it just resets back to max
    set_current_epoch(6);
    assert_eq!("10", call_read_taps_remaining_epoch(&faucet),
               "New epoch should increase to max only");

    // Make sure we are unprivileged bob before the read method tests
    // to ensure that they are available to everyone
    set_default_account(&bob);

    // Check that funding read works
    let funding = call_read_funding(&faucet);
    assert_eq!(RADIX_TOKEN, funding.0, "Funding should be in XRD");
    assert_eq!("139", funding.1, "Funding should be reported correctly");

    // Check that config read works
    let config = call_read_config(&faucet);
    assert_eq!("1", config.0, "Tap amount should be one");
    assert_eq!("10u64", config.1.unwrap(), "Taps per epoch should be ten");

    // Check that the admin badge reports what we expect
    assert_eq!(faucet.admin_badge_addr.as_ref().unwrap(),
               &call_read_admin_badge_address(&faucet),
               "Should be the same admin badge address as we started with");

    // Check that there's no taker badges
    let result = std::panic::catch_unwind(||
                                          call_read_taker_badge_address(&faucet));
    assert!(result.is_err(),
            "There should be no taker badges in this faucet instance");
}

/// Alice creates several faucets with taker badges and puts them to the test
#[test]
fn test_scenario_2() {
    reset_sim();
    let blueprint_addr = publish_faucet_component();

    // Alice owns the faucet
    let alice = create_account();
    let taker_badge_addr = new_badge_fixed("Test taker badge", "TTB", "20");
    let faucet = instantiate_faucet(&alice.address, &blueprint_addr,
                                    Some(&taker_badge_addr),
                                    RADIX_TOKEN, "100",
                                    "1", "Some(10u64)",
                                    true, false);

    assert_eq!("100", get_faucet_balance(&faucet),
               "Faucet should start with 100 tokens");
    assert_eq!(&taker_badge_addr, &call_read_taker_badge_address(&faucet),
               "Faucet with taker badge should report the correct resource");

    // Bob is a legitimate user of the faucet so Alice gives him a taker badge
    let bob = create_account();
    transfer_tokens(&bob, &taker_badge_addr, "1");
    assert_eq!("1", get_balance(&bob, &taker_badge_addr),
               "Bob should have a taker badge");

    // Charlie is a leecher nobody wants around so he doesn't get a taker badge
    let charlie = create_account();

    // Bob tries to take some without using his badge
    set_default_account(&bob);
    let result = std::panic::catch_unwind(||
                                          call_gimme(&faucet));
    assert!(result.is_err(),
            "Don't give funds when no badge presented");
    assert_eq!(get_balance(&bob, RADIX_TOKEN), "1000000",
               "Bob should still have his starting amount of XRD");
    assert_eq!("100", get_faucet_balance(&faucet),
               "Faucet should still have its starting funds");

    // Having received instructions, Bob now uses his badge
    set_default_account(&bob);
    call_gimme_with_badge(&faucet, &bob, &taker_badge_addr);
    assert_eq!(get_balance(&bob, RADIX_TOKEN), "1000001",
               "Bob should be a little bit richer now");
    assert_eq!("99", get_faucet_balance(&faucet),
               "Faucet should be a little bit poorer now");

    // Charlie also tries to take some without using his badge
    set_default_account(&charlie);
    let result = std::panic::catch_unwind(||
                                          call_gimme(&faucet));
    assert!(result.is_err(),
            "Don't give funds when no badge presented");
    assert_eq!(get_balance(&charlie, RADIX_TOKEN), "1000000",
               "Charlie should still have his starting amount of XRD");
    assert_eq!("99", get_faucet_balance(&faucet),
               "Faucet should be untouched");

    // Not satisfied, Charlie now tries to pretend he has a badge
    set_default_account(&charlie);
    let result = std::panic::catch_unwind(||
                                          call_gimme_with_badge(&faucet, &charlie, &taker_badge_addr));
    assert!(result.is_err(),
            "Don't give funds when no badge available");
    assert_eq!(get_balance(&charlie, RADIX_TOKEN), "1000000",
               "Charlie should still have his starting amount of XRD");
    assert_eq!("99", get_faucet_balance(&faucet),
               "Faucet should be untouched");

    // Charlie should be able to call public functions though
    set_default_account(&charlie);
    assert_eq!(&taker_badge_addr, &call_read_taker_badge_address(&faucet),
               "Charlie should be able to read taker badge address");

    // This faucet is set to only allow admin to refill it
    
    // Charlie should not be allowed to fill the faucet
    set_default_account(&charlie);
    let result = std::panic::catch_unwind(||
                                          call_fill(&faucet, &charlie, RADIX_TOKEN, "100"));
    assert!(result.is_err(),
            "Charlie can't fill without presenting badge when no stranger fill allowed");

    let result = std::panic::catch_unwind(||
                                          call_fill_with_badge(&faucet, &charlie,
                                                               &faucet.admin_badge_addr.as_ref().unwrap(),
                                                               RADIX_TOKEN, "100"));
    assert!(result.is_err(),
            "Charlie can't fill without a badge when no stranger fill allowed");

    // Bob also shouldn't be allowed, even if he presents a taker badge
    set_default_account(&bob);
    let result = std::panic::catch_unwind(||
                                          call_fill_with_badge(&faucet, &bob,
                                                               &taker_badge_addr,
                                                               RADIX_TOKEN, "100"));
    assert!(result.is_err(),
            "Bob can't fill with taker badge when no stranger fill allowed");

    // Alice can fill since she's admin
    set_default_account(&alice);
    call_fill_with_badge(&faucet, &alice,
                         &faucet.admin_badge_addr.as_ref().unwrap(),
                         RADIX_TOKEN, "200");
    assert_eq!("999700", get_balance(&alice, RADIX_TOKEN),
               "Alice should be down even more XRD");
    assert_eq!("299", get_faucet_balance(&faucet),
               "The faucet should be richer than ever");

    // Alice wants to test the faucet but in her confusion uses the wrong badge
    set_default_account(&alice);
    let result = std::panic::catch_unwind(||
                                          call_gimme_with_badge(&faucet, &alice,
                                                                &faucet.admin_badge_addr.as_ref().unwrap()));
    assert!(result.is_err(),
            "Alice shouldn't be allowed to tap with admin badge");
    // She'll need to use her taker badge instead
    call_gimme_with_badge(&faucet, &alice, &taker_badge_addr);
    assert_eq!("999701", get_balance(&alice, RADIX_TOKEN),
               "Alice should have some of her XRD back");
    assert_eq!("298", get_faucet_balance(&faucet),
               "The faucet should be one XRD down");

    // Bob wants to empty out the faucet, thinking he's tricky for using his taker badge this time
    set_default_account(&bob);
    let result = std::panic::catch_unwind(||
                                          call_empty(&faucet, &bob,
                                                     Some(&taker_badge_addr)));
    assert!(result.is_err(),
            "Bob shouldn't be able to steal funds using his taker badge");


    // This has been a raging success so Alice decides to set up another faucet,
    // for her new project. It uses the same taker badge as her other faucet
    // so she can more easily pull her existing community into her new project.
    set_default_account(&alice);
    let token_asc_addr = new_token_fixed("Alice's Shitcoin", "ASC", "100000000000");
    let faucet_asc = instantiate_faucet(&alice.address, &blueprint_addr,
                                    Some(&taker_badge_addr),
                                    &token_asc_addr, "100000",
                                    "1000", "Some(10u64)",
                                    false, true);

    // Bob eagerly gets his ASC using his existing taker badge
    set_default_account(&bob);
    call_gimme_with_badge(&faucet_asc, &bob, &taker_badge_addr);
    assert_eq!(get_balance(&bob, &token_asc_addr), "1000",
               "Bob should now have some shitcoin");
    assert_eq!("99000", get_faucet_balance(&faucet_asc),
               "Shitcoin faucet should be a little bit poorer now");


    // Alice also wants to set up a more exclusive faucet for her close
    // friends and family, with a different taker badge
    set_default_account(&alice);
    let taker_badge_elite_addr = new_badge_fixed("Elite taker badge", "ETB", "5");
    let faucet_asc_elite = instantiate_faucet(&alice.address, &blueprint_addr,
                                              Some(&taker_badge_elite_addr),
                                              &token_asc_addr, "10000000",
                                              "100000", "Some(10u64)",
                                              false, true);
    assert_eq!(&taker_badge_elite_addr, &call_read_taker_badge_address(&faucet_asc_elite),
               "Our taker badge should be the one we asked for");

    // Dave is a close personal friend and gets an elite taker badge
    let dave = create_account();
    transfer_tokens(&dave, &taker_badge_elite_addr, "1");
    assert_eq!("1", get_balance(&dave, &taker_badge_elite_addr),
               "Dave should have a taker badge");
    
    // Bob is a close friend, right? No he's not.
    set_default_account(&bob);
    let result = std::panic::catch_unwind(||
                                          call_gimme_with_badge(&faucet_asc_elite,
                                                                &bob, &taker_badge_addr));
    assert!(result.is_err(),
            "Bob shouldn't be able to use his commoner taker badge with the elite faucet");
    let result = std::panic::catch_unwind(||
                                          call_gimme_with_badge(&faucet_asc_elite,
                                                                &bob, &taker_badge_elite_addr));
    assert!(result.is_err(),
            "Bob shouldn't have the elite taker badge");
    assert_eq!(get_balance(&bob, &token_asc_addr), "1000",
               "Bob shouldn't have gotten any more");
    assert_eq!("10000000", get_faucet_balance(&faucet_asc_elite),
               "Shitcoin faucet should still have its starting stash");

    // Dave gets his
    // Having received instructions, Bob now uses his badge
    set_default_account(&dave);
    call_gimme_with_badge(&faucet_asc_elite, &dave, &taker_badge_elite_addr);
    assert_eq!(get_balance(&dave, &token_asc_addr), "100000",
               "Dave should now have some of Alice's shitcoin");
    assert_eq!("9900000", get_faucet_balance(&faucet_asc_elite),
               "Shitcoin faucet should be a little bit poorer");

    // Bob tries to bribe his way in by putting XRD into the ASC elite faucet,
    // which cannot be done because it only takes ASC
    set_default_account(&bob);
    let bobs_xrd_balance = get_balance(&bob, RADIX_TOKEN);
    let faucet_balance = get_faucet_balance(&faucet_asc_elite);
    let result = std::panic::catch_unwind(||
                                          call_fill(&faucet_asc_elite, &bob,
                                                    RADIX_TOKEN, "200"));
    assert!(result.is_err(),
            "Shouldn't be possible to put XRD into an ASC faucet");
    assert_eq!(bobs_xrd_balance, get_balance(&bob, RADIX_TOKEN),
               "Bob shouldn't have lost anything");
    assert_eq!(faucet_balance, get_faucet_balance(&faucet_asc_elite),
               "The faucet should be the same");
    
}
