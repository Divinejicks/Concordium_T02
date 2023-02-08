//! # A Concordium V1 smart contract
use concordium_std::*;
use core::fmt::Debug;

type DonationLocation = String;

/// Your smart contract state.
#[derive(Serialize, SchemaType, Clone)]
pub struct State {
    number_of_donors: u32,
    state_of_donation: StateOfDonation,
    donation_locations: Vec<DonationLocation>,
    end_time: Timestamp,
}

#[derive(Serialize, SchemaType, PartialEq, Eq, Debug, Clone, Copy)]
enum StateOfDonation {
    Open,
    Closed,
}

#[derive(Serialize, SchemaType)]
struct InitParameter {
    donation_locations: Vec<DonationLocation>,
    end_time: Timestamp,
}

/// Init function that creates a new smart contract.
#[init(contract = "donation", parameter = "InitParameter")]
fn init<S: HasStateApi>(
    ctx: &impl HasInitContext,
    _state_builder: &mut StateBuilder<S>,
) -> InitResult<State> {
    let param : InitParameter = ctx.parameter_cursor().get()?;

    Ok(State {
        number_of_donors: 0,
        state_of_donation: StateOfDonation::Open,
        donation_locations: param.donation_locations,
        end_time: param.end_time,
    })
}

/// Your smart contract errors.
#[derive(Debug, PartialEq, Eq, Reject, Serial, SchemaType)]
enum Error {
    /// Failed parsing the parameter.
    #[from(ParseError)]
    ParseParamsError,
    DonationHasEnded,
    DonationClosed,
    InvalidDonationLocation,
}

// Donating 
#[receive(
    contract = "donation",
    name = "donate",
    error = "Error",
    parameter = "DonationLocation",
    payable,
    mutable
)]
fn donate<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State, StateApiType = S>,
    _amount: Amount,
) -> Result<(), Error> {
    // checking for the end time
    if host.state().end_time < ctx.metadata().slot_time() {
        return Err(Error::DonationHasEnded);
    }

    // checking if donation is closed
    if host.state().state_of_donation == StateOfDonation::Closed {
        return Err(Error::DonationClosed);
     }

    // checking for the location the person is donating from
    let donation_location: DonationLocation = ctx.parameter_cursor().get()?;
    let _location_index = match host
        .state()
        .donation_locations
        .iter()
        .position(|location| *location == donation_location)
        {
            Some(idx) => idx as u32,
            None => return Err(Error::InvalidDonationLocation),
        };

    Ok(())
}

// Closing the donation
#[receive(contract = "donation", name = "close", mutable)]
fn close<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State, StateApiType = S>,
) -> ReceiveResult<()> {

    let owner = ctx.owner();
    let sender = ctx.sender();

    ensure!(sender.matches_account(&owner));
    ensure!(host.state().state_of_donation == StateOfDonation::Open);

    host.state_mut().state_of_donation = StateOfDonation::Closed;

    // transfering the balance to the owner
    let balance = host.self_balance();
    
    Ok(host.invoke_transfer(&owner, balance)?)
}

// Closing the donation
#[receive(contract = "donation", name = "open", mutable)]
fn open<S: HasStateApi>(
    ctx: &impl HasReceiveContext,
    host: &mut impl HasHost<State, StateApiType = S>,
) -> ReceiveResult<()> {

    let owner = ctx.owner();
    let sender = ctx.sender();

    ensure!(sender.matches_account(&owner));
    ensure!(host.state().state_of_donation == StateOfDonation::Closed);

    host.state_mut().state_of_donation = StateOfDonation::Open;
    Ok(())
}

#[derive(Serialize, SchemaType)]
struct DonationView {
    number_donors: u32,
    state_donation: StateOfDonation,
    time: Timestamp,
    balance: Amount,
}


/// View function that returns the content of the state.
#[receive(contract = "donation", name = "view", return_value = "DonationView")]
fn view<S: HasStateApi>(
    _ctx: &impl HasReceiveContext,
    host: &impl HasHost<State, StateApiType = S>,
) -> ReceiveResult<DonationView> {
    let state = host.state();
    let number_donors: u32 = state.number_of_donors.clone();
    let state_donation: StateOfDonation = state.state_of_donation.clone();
    let time: Timestamp = state.end_time;
    let balance = host.self_balance();
    Ok(DonationView {
        number_donors,
        state_donation,
        time,
        balance,
    })
}



#[concordium_cfg_test]
mod tests {
    use super::*;
    use test_infrastructure::*;

    const ACC: AccountAddress = AccountAddress([0u8; 32]);

    #[test]
    fn test_donate() {
        // arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(Address::Account(ACC));
        ctx.set_metadata_slot_time(Timestamp::from_timestamp_millis(0));
        let donation_location = "CM";
        let parameter = to_bytes(&donation_location);
        ctx.set_parameter(&parameter);
        let amount = Amount::from_micro_ccd(100);

        let state = State {
            number_of_donors: 0,
            state_of_donation: StateOfDonation::Open,
            donation_locations: vec!["GE".to_string(), "CM".to_string(), "IT".to_string(), "FR".to_string()],
            end_time: Timestamp::from_timestamp_millis(10000),
        };

        let mut host = TestHost::new(state, TestStateBuilder::new());

        // act
        let result = donate(&ctx, &mut host, amount);

        // assert
        assert!(result.is_ok(), "Inserting CCD results in error");

        assert_eq!(
            host.state().state_of_donation,
            StateOfDonation::Open,
            "State of donation should still be open"
        );
    }

    #[test]
    fn test_donate_wrong_location() {
        // arrange
        let mut ctx = TestReceiveContext::empty();
        ctx.set_sender(Address::Account(ACC));
        ctx.set_metadata_slot_time(Timestamp::from_timestamp_millis(0));
        let donation_location = "USA";
        let parameter = to_bytes(&donation_location);
        ctx.set_parameter(&parameter);
        let amount = Amount::from_micro_ccd(100);

        let state = State {
            number_of_donors: 0,
            state_of_donation: StateOfDonation::Open,
            donation_locations: vec!["GE".to_string(), "CM".to_string(), "IT".to_string(), "FR".to_string()],
            end_time: Timestamp::from_timestamp_millis(10000),
        };

        let mut host = TestHost::new(state, TestStateBuilder::new());

        // act
        let result = donate(&ctx, &mut host, amount);

        // assert
        assert!(result.is_err(), "Failed due to wrong location");
    }

    #[test]
    fn test_close() {
        // arrange
        let mut ctx = TestReceiveContext::empty();
        let owner = AccountAddress([0u8; 32]);
        ctx.set_owner(owner);
        let sender = Address::Account(owner);
        ctx.set_sender(sender);
        let balance = Amount::from_micro_ccd(100);

        let state = State {
            number_of_donors: 0,
            state_of_donation: StateOfDonation::Open,
            donation_locations: vec!["GE".to_string(), "CM".to_string(), "IT".to_string(), "FR".to_string()],
            end_time: Timestamp::from_timestamp_millis(10000),
        };
        

        let mut host = TestHost::new(state, TestStateBuilder::new());
        host.set_self_balance(balance);
        // act
        let result = close(&ctx, &mut host);

        // assert
        assert!(result.is_ok(), "Failed to close donation.");
        assert_eq!(host.state().state_of_donation, StateOfDonation::Closed, "State of donation should be closed.");
        assert_eq!(
            host.get_transfers(),
            [(owner, balance)],
            "wrong transfers."
        );
    }

    #[test]
    fn test_open() {
        // arrange
        let mut ctx = TestReceiveContext::empty();
        let owner = AccountAddress([0u8; 32]);
        ctx.set_owner(owner);
        let sender = Address::Account(owner);
        ctx.set_sender(sender);
        let balance = Amount::from_micro_ccd(100);

        let state = State {
            number_of_donors: 0,
            state_of_donation: StateOfDonation::Open,
            donation_locations: vec!["GE".to_string(), "CM".to_string(), "IT".to_string(), "FR".to_string()],
            end_time: Timestamp::from_timestamp_millis(10000),
        };
        

        let mut host = TestHost::new(state, TestStateBuilder::new());
        host.set_self_balance(balance);
        // act
        let result = close(&ctx, &mut host);

        // assert
        assert!(result.is_ok(), "Failed to close donation.");
        assert_eq!(host.state().state_of_donation, StateOfDonation::Closed, "State of donation should be closed.");
        assert_eq!(
            host.get_transfers(),
            [(owner, balance)],
            "wrong transfers."
        );

        // open
        let openResult = open(&ctx, &mut host);
        assert!(result.is_ok(), "Failed to open donation.");
        assert_eq!(host.state().state_of_donation, StateOfDonation::Open, "State of donation should be open.");
    }
}