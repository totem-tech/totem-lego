//                              Næ§@@@ÑÉ©
//                        æ@@@@@@@@@@@@@@@@@@
//                    Ñ@@@@?.?@@@@@@@@@@@@@@@@@@@N
//                 ¶@@@@@?^%@@.=@@@@@@@@@@@@@@@@@@@@
//               N@@@@@@@?^@@@»^@@@@@@@@@@@@@@@@@@@@@@
//               @@@@@@@@?^@@@».............?@@@@@@@@@É
//              Ñ@@@@@@@@?^@@@@@@@@@@@@@@@@@@'?@@@@@@@@Ñ
//              @@@@@@@@@?^@@@»..............»@@@@@@@@@@
//              @@@@@@@@@?^@@@»^@@@@@@@@@@@@@@@@@@@@@@@@
//              @@@@@@@@@?^ë@@&.@@@@@@@@@@@@@@@@@@@@@@@@
//               @@@@@@@@?^´@@@o.%@@@@@@@@@@@@@@@@@@@@©
//                @@@@@@@?.´@@@@@ë.........*.±@@@@@@@æ
//                 @@@@@@@@?´.I@@@@@@@@@@@@@@.&@@@@@N
//                  N@@@@@@@@@@ë.*=????????=?@@@@@Ñ
//                    @@@@@@@@@@@@@@@@@@@@@@@@@@@¶
//                        É@@@@@@@@@@@@@@@@Ñ¶
//                             Næ§@@@ÑÉ©

// Copyright 2020 Chris D'Costa
// This file is part of Totem Live Accounting.
// Author Chris D'Costa email: chris.dcosta@totemaccounting.com

// Totem is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Totem is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Totem.  If not, see <http://www.gnu.org/licenses/>.

// Locks prefunded amounts into the runtime.
//
// This module functions as a pseudo-escrow module, holding funds for a specified period of time and or for a specific beneficiary.
// In addition to locking funds until a deadline, this module also updates the accounting ledger showing that the assets have moved.
// There is no automatic release of funds from the locked state so requires that the either the deadline to have past to allow withdrawal
// or the intervention of the permitted party to withdraw the funds.
//
// For the initial use of this prefunding module the intended beneficiary is identified by AccountId.
// In a later version there may be no intended beneficiary (for example for marketplace transactions)
// and therefore the funds may be locked until a cadidate secures the funds.
//
// A further scenario is forseen where a dispute resolution method that relies upon an independent validator
// is required to set the lock-release state.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    dispatch::EncodeLike,
    fail,
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement, LockIdentifier, WithdrawReasons},
};
use frame_system::pallet_prelude::*;
use pallet_balances::totem::TotemLockableCurrency;

use sp_runtime::traits::{Convert, Hash};
use sp_std::{prelude::*, vec};

use totem_common::traits::{accounting::Posting, prefunding::Encumbrance};
use totem_common::types::ComparisonAmounts;
use totem_common::{ok, StorageMapExt};

type AccountOf<T> = <<T as pallet_balances::Config>::Accounting as Posting<
    <T as frame_system::Config>::AccountId,
    <T as frame_system::Config>::Hash,
    <T as frame_system::Config>::BlockNumber,
    <T as pallet_balances::Config>::Balance,
>>::Account;

type AccountBalanceOf<T> = <<T as pallet_balances::Config>::Accounting as Posting<
    <T as frame_system::Config>::AccountId,
    <T as frame_system::Config>::Hash,
    <T as frame_system::Config>::BlockNumber,
    <T as pallet_balances::Config>::Balance,
>>::LedgerBalance;

type CurrencyBalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Decode, Encode, PartialEq, Eq)]
pub enum LockStatus {
    Unlocked = 0,
    Locked = 1,
}
impl EncodeLike<LockStatus> for bool {}

/// Generic Status for whatever the HashReference refers
//TODO
pub type Status = u16;

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    #[pallet::pallet]
    #[pallet::generate_store(trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn prefunding)]
    /// Bonsai Storage
    pub type Prefunding<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, (CurrencyBalanceOf<T>, T::BlockNumber)>;

    /* Hacky workaround for inability of RPC to query transaction by hash */

    #[pallet::storage]
    #[pallet::getter(fn prefunding_hash_owner)]
    /// Maps to current block number allows interrogation of errors.
    pub type PrefundingHashOwner<T: Config> =
        StorageMap<_, Blake2_128Concat, T::Hash, (T::AccountId, LockStatus, T::AccountId, LockStatus)>;

    #[pallet::storage]
    #[pallet::getter(fn owner_prefunding_hash_list)]
    /// Future block number beyond which the Hash should deleted.
    pub type OwnerPrefundingHashList<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Vec<T::Hash>>;

    #[pallet::storage]
    #[pallet::getter(fn reference_status)]
    /// Tracking to ensure that we can perform housekeeping on finalization of block.
    pub type ReferenceStatus<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, Status>;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_balances::Config + pallet_timestamp::Config + pallet_accounting::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: Currency<Self::AccountId> + TotemLockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type PrefundingConversions: Convert<AccountBalanceOf<Self>, u128>
            + Convert<AccountBalanceOf<Self>, CurrencyBalanceOf<Self>>
            + Convert<CurrencyBalanceOf<Self>, AccountBalanceOf<Self>>
            + Convert<Vec<u8>, LockIdentifier>
            + Convert<u64, AccountOf<Self>>
            + Convert<u64, CurrencyBalanceOf<Self>>
            + Convert<u32, Self::BlockNumber>
            + Convert<i128, AccountBalanceOf<Self>>
            + Convert<u128, AccountBalanceOf<Self>>
            + Convert<u128, i128>
            + Convert<AccountBalanceOf<Self>, i128>
            + Convert<CurrencyBalanceOf<Self>, u128>;
    }

    #[pallet::error]
    pub enum Error<T> {
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed1,
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed2,
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed3,
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed4,
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed5,
        /// You are not the owner or the beneficiary
        ErrorLockNotAllowed6,
        /// Not enough funds to prefund
        ErrorInsufficientPreFunds,
        /// Cannot set this state
        ErrorWrongState1,
        /// Cannot set this state
        ErrorWrongState2,
        /// Cannot set this state
        ErrorWrongState3,
        /// Cannot set this state
        ErrorWrongState4,
        /// Cannot set this state
        ErrorWrongState5,
        /// Funds already locked for intended purpose by both parties.
        ErrorNotAllowed1,
        /// Not the beneficiary
        ErrorNotAllowed2,
        /// Not the owner
        ErrorNotAllowed3,
        /// This function should not be used for this state
        ErrorNotAllowed4,
        /// Funds locked for intended purpose by both parties.
        ErrorNotAllowed5,
        /// Funds locked for beneficiary.
        ErrorNotAllowed6,
        /// The demander has not approved the work yet!
        ErrorNotApproved,
        /// The demander has not approved the work yet!
        ErrorNotApproved2,
        /// Deadline not yet passed. Wait a bit longer!
        ErrorDeadlineInPlay,
        /// Funds locked for intended purpose by both parties.
        ErrorFundsInPlay,
        /// Funds locked for intended purpose by both parties.
        ErrorFundsInPlay2,
        /// You are not the owner of the hash!
        ErrorNotOwner,
        /// You are not the owner of the hash!
        ErrorNotOwner2,
        /// This hash already exists!
        ErrorHashExists,
        /// Hash does not exist
        ErrorHashDoesNotExist,
        /// Hash does not exist
        ErrorHashDoesNotExist2,
        /// Hash does not exist
        ErrorHashDoesNotExist3,
        /// Deadline is too short! Must be at least 48 hours
        ErrorShortDeadline,
        /// Deposit was not taken
        ErrorPrefundNotSet,
        /// An error occured posting to accounts - prefunding for...
        ErrorInAccounting1,
        /// An error occured posting to accounts - send simple invoice
        ErrorInAccounting2,
        /// An error occured posting to accounts - settle invoice
        ErrorInAccounting3,
        /// Did not set the status - prefunding for...
        ErrorSettingStatus1,
        /// Did not set the status - send simple invoice
        ErrorSettingStatus2,
        /// Error getting details from hash
        ErrorNoDetails,
        /// Error setting release state
        ErrorReleaseState,
        /// Error unlocking for beneficiary
        ErrorUnlocking,
        /// Error cancelling prefunding
        ErrorCancellingPrefund,
        /// Error getting prefunding details
        ErrorNoPrefunding,
        /// Cancelling prefunding failed for some reason
        ErrorCancelFailed,
        /// Cancelling prefunding failed for some reason
        ErrorCancelFailed2,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// This function reserves funds from the buyer for a specific vendor account (Closed Order). It is used when an order is created.
        /// Quatity is not relevant
        /// The prefunded amount remains as an asset of the buyer until the order is accepted
        /// Updates only the accounts of the buyer
        #[pallet::weight(0/*TODO*/)]
        fn prefund_someone(
            origin: OriginFor<T>,
            beneficiary: T::AccountId,
            amount: u128,
            deadline: T::BlockNumber,
            tx_uid: T::Hash,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            // check that the beneficiary is not the sender
            ensure!(who != beneficiary, "Beneficiary must be another account");
            let prefunding_hash: T::Hash = Self::get_pseudo_random_hash(who.clone(), beneficiary.clone());

            Self::prefunding_for(who, beneficiary, amount.into(), deadline, prefunding_hash, tx_uid)
        }

        /// Creates a single line simple invoice without taxes, tariffs or commissions
        /// This invoice is associated with a prefunded order - therefore needs to provide the hash reference of the order
        /// Updates the accounting for the vendor and the customer
        #[pallet::weight(0/*TODO*/)]
        fn invoice_prefunded_order(
            origin: OriginFor<T>,
            payer: T::AccountId,
            amount: i128,
            reference: T::Hash,
            uid: T::Hash,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::send_simple_invoice(who.clone(), payer.clone(), amount, reference, uid)
        }

        /// Buyer pays a prefunded order. Needs to supply the correct hash reference
        /// Updates bother the buyer and the vendor accounts
        #[pallet::weight(0/*TODO*/)]
        fn pay_prefunded_invoice(origin: OriginFor<T>, reference: T::Hash, uid: T::Hash) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::settle_prefunded_invoice(who.clone(), reference, uid)
        }

        /// Is used by the buyer to recover funds if the vendor does not accept the order by the deadline
        #[pallet::weight(0/*TODO*/)]
        fn cancel_prefunded_closed_order(
            origin: OriginFor<T>,
            reference: T::Hash,
            uid: T::Hash,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::unlock_funds_for_owner(who.clone(), reference, uid)
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PrefundingCancelled(T::AccountId, T::Hash),
        PrefundingLockSet(T::Hash),
        PrefundingCompleted(T::Hash),
        InvoiceIssued(T::Hash),
        InvoiceSettled(T::Hash),
    }
}

impl<T: Config> Pallet<T> {
    /// Reserve the prefunding deposit
    fn set_prefunding(
        s: T::AccountId,
        c: AccountBalanceOf<T>,
        d: T::BlockNumber,
        h: T::Hash,
        _u: T::Hash,
    ) -> DispatchResultWithPostInfo {
        // Prepare make sure we are not taking the deposit again
        if ReferenceStatus::<T>::contains_key(&h) {
            fail!(Error::<T>::ErrorHashExists);
        }

        // You cannot prefund any amount unless you have at least at balance of 1618 units + the amount you want to prefund
        // Ensure that the funds can be subtracted from sender's balance without causing the account to be destroyed by the existential deposit
        let min_balance: ComparisonAmounts = 1618u128;
        let current_balance: ComparisonAmounts =
            <T::PrefundingConversions as Convert<CurrencyBalanceOf<T>, u128>>::convert(T::Currency::free_balance(&s));
        let prefund_amount: ComparisonAmounts =
            <T::PrefundingConversions as Convert<AccountBalanceOf<T>, u128>>::convert(c.clone());
        let minimum_amount = min_balance + prefund_amount;

        if current_balance >= minimum_amount {
            let converted_amount: CurrencyBalanceOf<T> = T::PrefundingConversions::convert(c.clone());
            // Lock the amount from the sender and set deadline
            T::Currency::totem_set_lock(Self::get_prefunding_id(h), &s, converted_amount, d, WithdrawReasons::RESERVE);
        } else {
            fail!(Error::<T>::ErrorInsufficientPreFunds);
        }

        ok()
    }

    /// Generate Prefund Id from hash  
    fn get_prefunding_id(hash: T::Hash) -> LockIdentifier {
        // Convert Hash to ID using first 8 bytes of hash
        <T::PrefundingConversions as Convert<Vec<u8>, LockIdentifier>>::convert(hash.encode())
    }

    /// generate reference hash
    fn get_pseudo_random_hash(sender: T::AccountId, recipient: T::AccountId) -> T::Hash {
        let tuple = (sender, recipient);
        let input = (
            tuple,
            pallet_timestamp::Pallet::<T>::get(),
            sp_io::offchain::random_seed(),
            frame_system::Pallet::<T>::extrinsic_index(),
            frame_system::Pallet::<T>::block_number(),
        );

        T::Hashing::hash(input.encode().as_slice()) // default hash BlakeTwo256
    }

    /// check hash exists and is valid
    fn reference_valid(h: T::Hash) -> bool {
        match ReferenceStatus::<T>::get(&h) {
            Some(0) | Some(1) | Some(100) | Some(200) | Some(300) | Some(400) => true,
            _ => false,
        }
    }

    /// Prefunding deadline passed?
    fn prefund_deadline_passed(h: T::Hash) -> bool {
        match Self::prefunding(&h) {
            Some((_, deadline)) if deadline < frame_system::Pallet::<T>::block_number() => true,
            _ => false,
        }
    }

    /// Gets the state of the locked funds. The hash needs to be prequalified before passing in as no checks performed here.
    fn get_release_state(h: T::Hash) -> (LockStatus, LockStatus) {
        let owners = Self::prefunding_hash_owner(&h).unwrap(); //TODO

        (owners.1, owners.3)
    }

    /// cancel lock for owner
    fn cancel_prefunding_lock(o: T::AccountId, h: T::Hash, s: Status) -> DispatchResultWithPostInfo {
        // funds can be unlocked for the owner
        // convert hash to lock identifyer
        let prefunding_id = Self::get_prefunding_id(h);
        // unlock the funds
        T::Currency::totem_remove_lock(prefunding_id, &o);
        // perform cleanup removing all reference hashes. No accounting posting have been made, so no cleanup needed there
        Prefunding::<T>::remove(&h);
        PrefundingHashOwner::<T>::remove(&h);
        ReferenceStatus::<T>::insert(&h, s); // This sets the status but does not remove the hash
        OwnerPrefundingHashList::<T>::mutate_(&o, |owner_prefunding_hash_list| {
            owner_prefunding_hash_list.retain(|e| e != &h)
        });

        // Issue event
        Self::deposit_event(Event::PrefundingCancelled(o, h));

        ok()
    }

    /// unlock & pay beneficiary with funds transfer and account updates (settlement of invoice)
    fn unlock_funds_for_beneficiary(o: T::AccountId, h: T::Hash, _u: T::Hash) -> DispatchResultWithPostInfo {
        use LockStatus::*;

        if Self::reference_valid(h) == false {
            fail!(Error::<T>::ErrorHashDoesNotExist);
        }

        if Self::check_ref_beneficiary(o.clone(), h) == false {
            fail!(Error::<T>::ErrorNotOwner);
        }

        // TODO this should return the details otherwise there is second read later in the process
        match Self::get_release_state(h) {
            // submitted, but not yet accepted
            (Locked, Unlocked) => fail!(Error::<T>::ErrorNotApproved),
            (Locked, Locked) => fail!(Error::<T>::ErrorFundsInPlay),
            // Owner has approved now get status of hash. Only allow if invoiced.
            // Note handling the account posting is done outside of this function
            (Unlocked, Locked) => {
                match ReferenceStatus::<T>::get(&h) {
                    Some(400) => {
                        // get details of lock
                        let details = Self::prefunding_hash_owner(&h).ok_or("Error fetching details")?;
                        // get details of prefunding
                        let prefunding = Self::prefunding(&h).ok_or("Error getting prefunding details")?;
                        // Cancel prefunding lock
                        let status: Status = 500; // Settled
                        Self::cancel_prefunding_lock(details.0.clone(), h, status)?;
                        // transfer to beneficiary.
                        // TODO when currency conversion is implemnted the payment should be at the current rate for the currency
                        if let Err(_) =
                            T::Currency::transfer(&details.0, &o, prefunding.0, ExistenceRequirement::KeepAlive)
                        {
                            fail!("Error during transfer")
                        }
                    }
                    _ => fail!("Only allowed when status is Invoiced"),
                }
            }
            // Owner has been given permission by beneficiary to release funds
            (Unlocked, Unlocked) => fail!(Error::<T>::ErrorNotAllowed1),
        }

        ok()
    }

    /// set the status for the prefunding
    fn set_ref_status(h: T::Hash, s: Status) -> DispatchResultWithPostInfo {
        ReferenceStatus::<T>::insert(&h, s);

        ok()
    }

    // TODO Check should be made for available balances, and if the amount submitted is more than the invoice amount.
    /// Settles invoice by updates to various relevant accounts and transfer of funds
    fn settle_unfunded_invoice() -> DispatchResultWithPostInfo {
        fail!("TODO")
    }
}

pub use pallet::*;

impl<T: Config> Encumbrance<T::AccountId, T::Hash, T::BlockNumber> for Pallet<T> {
    type LockStatus = LockStatus;

    fn prefunding_for(
        who: T::AccountId,
        recipient: T::AccountId,
        amount: u128,
        deadline: T::BlockNumber,
        ref_hash: T::Hash,
        uid: T::Hash,
    ) -> DispatchResultWithPostInfo {
        // As amount will always be positive, convert for use in accounting
        let amount_converted: AccountBalanceOf<T> =
            <T::PrefundingConversions as Convert<u128, AccountBalanceOf<T>>>::convert(amount);
        // Convert this for the inversion
        let to_invert: i128 =
            <T::PrefundingConversions as Convert<AccountBalanceOf<T>, i128>>::convert(amount_converted.clone());
        // invert the amount
        let to_invert = to_invert * -1;
        let increase_amount: AccountBalanceOf<T> = amount_converted.clone();
        let decrease_amount: AccountBalanceOf<T> =
            <T::PrefundingConversions as Convert<i128, AccountBalanceOf<T>>>::convert(to_invert);
        let current_block = <frame_system::Pallet<T>>::block_number();
        // Prefunding is always recorded in the same block. It cannot be posted to another period
        let current_block_dupe = <frame_system::Pallet<T>>::block_number();
        let prefunding_hash: T::Hash = ref_hash.clone();
        // convert the account balanace to the currency balance (i128 -> u128)
        let currency_amount: CurrencyBalanceOf<T> = <T::PrefundingConversions as Convert<
            AccountBalanceOf<T>,
            CurrencyBalanceOf<T>,
        >>::convert(amount_converted.clone());
        // NEED TO CHECK THAT THE DEADLINE IS SENSIBLE!!!!
        // 48 hours is the minimum deadline. This is the minimum amountof time before the money can be reclaimed
        let minimum_deadline: T::BlockNumber =
            current_block + <T::PrefundingConversions as Convert<u32, T::BlockNumber>>::convert(11520_u32);
        if deadline < minimum_deadline {
            fail!(Error::<T>::ErrorShortDeadline);
        }
        let prefunded = (currency_amount, deadline.clone());
        let owners = (who.clone(), true, recipient.clone(), false);
        // manage the deposit
        if let Err(_) = Self::set_prefunding(who.clone(), amount_converted.clone(), deadline, prefunding_hash, uid) {
            fail!(Error::<T>::ErrorPrefundNotSet);
        }
        // Deposit taken at this point. Note that if an error occurs beyond here we need to remove the locked funds.
        // Buyer
        let account_1 = T::PrefundingConversions::convert(110_10005000_0000_u64); // debit  increase 110100050000000 Prefunding Account
        let account_2 = T::PrefundingConversions::convert(110_10004000_0000_u64); // credit decrease 110100040000000 XTX Balance
        let account_3 = T::PrefundingConversions::convert(360_60002000_0000_u64); // debit  increase 360600020000000 Runtime Ledger by Module
        let account_4 = T::PrefundingConversions::convert(360_60006000_0000_u64); // debit  increase 360600060000000 Runtime Ledger Control

        // Keys for posting
        let forward_keys = vec![
            (who.clone(), account_1, increase_amount, true, prefunding_hash, current_block, current_block_dupe),
            (who.clone(), account_2, decrease_amount, false, prefunding_hash, current_block, current_block_dupe),
            (who.clone(), account_3, increase_amount, true, prefunding_hash, current_block, current_block_dupe),
            (who.clone(), account_4, increase_amount, true, prefunding_hash, current_block, current_block_dupe),
        ];
        // Reversal keys in case of errors
        let reversal_keys = vec![
            (who.clone(), account_1, decrease_amount, false, prefunding_hash, current_block, current_block_dupe),
            (who.clone(), account_2, increase_amount, true, prefunding_hash, current_block, current_block_dupe),
            (who.clone(), account_3, decrease_amount, false, prefunding_hash, current_block, current_block_dupe),
            (who.clone(), account_4, decrease_amount, false, prefunding_hash, current_block, current_block_dupe),
        ];

        if let Err(_) = T::Accounting::handle_multiposting_amounts(forward_keys, reversal_keys) {
            fail!(Error::<T>::ErrorInAccounting1);
        }

        // Record Prefunding ownership and status
        PrefundingHashOwner::<T>::insert(&prefunding_hash, owners);
        Prefunding::<T>::insert(&prefunding_hash, prefunded);

        // Add reference hash to list of hashes
        OwnerPrefundingHashList::<T>::mutate_(&who, |owner_prefunding_hash_list| {
            owner_prefunding_hash_list.push(prefunding_hash)
        });

        // Submitted, Locked by sender.
        if let Err(_) = Self::set_ref_status(prefunding_hash, 1) {
            fail!(Error::<T>::ErrorSettingStatus1);
        }

        Self::deposit_event(Event::PrefundingCompleted(uid));

        ok()
    }

    /// Simple invoice. Does not include tax jurisdiction, tax amounts, freight, commissions, tariffs, discounts and other extended line item values
    /// must include a connection to the originating reference.
    /// Invoices cannot be made to parties that haven't asked for something identified by a valid hash
    fn send_simple_invoice(
        o: T::AccountId,
        p: T::AccountId,
        n: i128,
        h: T::Hash,
        u: T::Hash,
    ) -> DispatchResultWithPostInfo {
        // Validate that the hash is indeed assigned to the seller
        if Self::check_ref_beneficiary(o.clone(), h) == false {
            fail!(Error::<T>::ErrorNotAllowed2);
        }

        // Amount CAN be negative - this is therefore not an Invoice but a Credit Note!
        // The account postings are identical to an invoice, however we must also handle the refund immediately if possible.
        // In order to proceed with a credit note, validate that the vendor has sufficient funds.
        // If they do not have sufficient funds, the credit note can still be issued, but will remain outstanding until it is settled.
        // As amount will always be positive, convert for use in accounting
        let amount_converted: AccountBalanceOf<T> =
            <T::PrefundingConversions as Convert<i128, AccountBalanceOf<T>>>::convert(n);
        // invert the amount
        let inverted: i128 = n * -1;
        let increase_amount: AccountBalanceOf<T> = amount_converted.clone();
        let decrease_amount: AccountBalanceOf<T> =
            <T::PrefundingConversions as Convert<i128, AccountBalanceOf<T>>>::convert(inverted);
        let current_block = frame_system::Pallet::<T>::block_number();
        let current_block_dupe = frame_system::Pallet::<T>::block_number();

        // Seller
        let account_1 = T::PrefundingConversions::convert(110_10008000_0000_u64); // Debit  increase 110100080000000	Accounts receivable (Sales Control Account or Trade Debtor's Account)
        let account_2 = T::PrefundingConversions::convert(240_40001000_0000_u64); // Credit increase 240400010000000	Product or Service Sales
        let account_3 = T::PrefundingConversions::convert(360_60001000_0000_u64); // Debit  increase 360600010000000	Sales Ledger by Payer
        let account_4 = T::PrefundingConversions::convert(360_60005000_0000_u64); // Debit  increase 360600050000000	Sales Ledger Control

        // Buyer
        let account_5 = T::PrefundingConversions::convert(120_20003000_0000_u64); // Credit increase 120200030000000	Accounts payable
        let account_6 = T::PrefundingConversions::convert(250_50012000_0013_u64); // Debit  increase 250500120000013	Labour
        let account_7 = T::PrefundingConversions::convert(360_60003000_0000_u64); // Debit  increase 360600030000000	Purchase Ledger by Vendor
        let account_8 = T::PrefundingConversions::convert(360_60007000_0000_u64); // Debit  increase 360600070000000	Purchase Ledger Control

        // Keys for posting
        let forward_keys = vec![
            (o.clone(), account_1, increase_amount, true, h, current_block, current_block_dupe),
            (o.clone(), account_2, increase_amount, false, h, current_block, current_block_dupe),
            (o.clone(), account_3, increase_amount, true, h, current_block, current_block_dupe),
            (o.clone(), account_4, increase_amount, true, h, current_block, current_block_dupe),
            (p.clone(), account_5, increase_amount, false, h, current_block, current_block_dupe),
            (p.clone(), account_6, increase_amount, true, h, current_block, current_block_dupe),
            (p.clone(), account_7, increase_amount, true, h, current_block, current_block_dupe),
            (p.clone(), account_8, increase_amount, true, h, current_block, current_block_dupe),
        ];

        // Reversal keys in case of errors
        let reversal_keys = vec![
            (o.clone(), account_1, decrease_amount, false, h, current_block, current_block_dupe),
            (o.clone(), account_2, decrease_amount, true, h, current_block, current_block_dupe),
            (o.clone(), account_3, decrease_amount, false, h, current_block, current_block_dupe),
            (o.clone(), account_4, decrease_amount, false, h, current_block, current_block_dupe),
            (p.clone(), account_5, decrease_amount, true, h, current_block, current_block_dupe),
            (p.clone(), account_6, decrease_amount, false, h, current_block, current_block_dupe),
            (p.clone(), account_7, decrease_amount, false, h, current_block, current_block_dupe),
        ];

        if let Err(_) = T::Accounting::handle_multiposting_amounts(forward_keys, reversal_keys) {
            fail!(Error::<T>::ErrorInAccounting2);
        }

        // Add status processing
        let new_status: Status = 400; // invoiced(400), can no longer be accepted,
        if let Err(_) = Self::set_ref_status(h, new_status) {
            fail!(Error::<T>::ErrorSettingStatus2);
        }

        Self::deposit_event(Event::InvoiceIssued(u));

        ok()
    }

    // Settles invoice by unlocking funds and updates various relevant accounts and pays prefunded amount
    fn settle_prefunded_invoice(o: T::AccountId, h: T::Hash, uid: T::Hash) -> DispatchResultWithPostInfo {
        use LockStatus::*;

        // release state must be 11
        // sender must be owner
        // accounts updated before payment, because if there is an error then the accounting can be rolled back
        let (payer, beneficiary) = match Self::get_release_state(h) {
            // submitted, but not yet accepted
            (Locked, Unlocked) => fail!(Error::<T>::ErrorNotApproved2),
            (Locked, Locked) => {
                // Validate that the hash is indeed owned by the buyer
                if Self::check_ref_owner(o.clone(), h) == false {
                    fail!(Error::<T>::ErrorNotAllowed3);
                }

                // get beneficiary from hash
                let (_, _, details /*TODO better name*/, _) =
                    Self::prefunding_hash_owner(&h).ok_or(Error::<T>::ErrorNoDetails)?;
                // get prefunding amount for posting to accounts
                let (prefunded_amount, _) = Self::prefunding(&h).ok_or(Error::<T>::ErrorNoPrefunding)?;
                // convert to Account Balance type
                let amount: AccountBalanceOf<T> = <T::PrefundingConversions as Convert<
                    CurrencyBalanceOf<T>,
                    AccountBalanceOf<T>,
                >>::convert(prefunded_amount.into());
                // Convert for calculation
                let inverted =
                    -1 * <T::PrefundingConversions as Convert<AccountBalanceOf<T>, i128>>::convert(amount.clone());
                let increase_amount = amount;
                let decrease_amount =
                    <T::PrefundingConversions as Convert<i128, AccountBalanceOf<T>>>::convert(inverted);
                let current_block = frame_system::Pallet::<T>::block_number();
                let current_block_dupe = frame_system::Pallet::<T>::block_number();

                let account_1 = T::PrefundingConversions::convert(120_20003000_0000_u64); // 120200030000000	Debit  decrease Accounts payable
                let account_2 = T::PrefundingConversions::convert(110_10005000_0000_u64); // 110100050000000	Credit decrease Totem Runtime Deposit (Escrow)
                let account_3 = T::PrefundingConversions::convert(360_60002000_0000_u64); // 360600020000000	Credit decrease Runtime Ledger by Module
                let account_4 = T::PrefundingConversions::convert(360_60006000_0000_u64); // 360600060000000	Credit decrease Runtime Ledger Control
                let account_5 = T::PrefundingConversions::convert(360_60003000_0000_u64); // 360600030000000	Credit decrease Purchase Ledger by Vendor
                let account_6 = T::PrefundingConversions::convert(360_60007000_0000_u64); // 360600070000000	Credit decrease Purchase Ledger Control

                let account_7 = T::PrefundingConversions::convert(110_10004000_0000_u64); // 110100040000000	Debit  increase XTX Balance
                let account_8 = T::PrefundingConversions::convert(110_10008000_0000_u64); // 110100080000000	Credit decrease Accounts receivable (Sales Control Account or Trade Debtor's Account)
                let account_9 = T::PrefundingConversions::convert(360_60001000_0000_u64); // 360600010000000	Credit decrease Sales Ledger by Payer
                let account_10 = T::PrefundingConversions::convert(360_60005000_0000_u64); // 360600050000000	Credit decrease Sales Ledger Control

                // Keys for posting
                let forward_keys = vec![
                    // Buyer
                    (o.clone(), account_1, decrease_amount, true, h, current_block, current_block_dupe),
                    (o.clone(), account_2, decrease_amount, false, h, current_block, current_block_dupe),
                    (o.clone(), account_3, decrease_amount, false, h, current_block, current_block_dupe),
                    (o.clone(), account_4, decrease_amount, false, h, current_block, current_block_dupe),
                    (o.clone(), account_5, decrease_amount, false, h, current_block, current_block_dupe),
                    (o.clone(), account_6, decrease_amount, false, h, current_block, current_block_dupe),
                    // Seller
                    (details.clone(), account_7, increase_amount, true, h, current_block, current_block_dupe),
                    (details.clone(), account_8, decrease_amount, false, h, current_block, current_block_dupe),
                    (details.clone(), account_9, decrease_amount, false, h, current_block, current_block_dupe),
                    (details.clone(), account_10, decrease_amount, false, h, current_block, current_block_dupe),
                ];

                // Reversal keys in case of errors
                let reversal_keys = vec![
                    // Buyer
                    (o.clone(), account_1, increase_amount, false, h, current_block, current_block_dupe),
                    (o.clone(), account_2, increase_amount, true, h, current_block, current_block_dupe),
                    (o.clone(), account_3, increase_amount, true, h, current_block, current_block_dupe),
                    (o.clone(), account_4, increase_amount, true, h, current_block, current_block_dupe),
                    (o.clone(), account_5, increase_amount, true, h, current_block, current_block_dupe),
                    (o.clone(), account_6, increase_amount, true, h, current_block, current_block_dupe),
                    // Seller
                    (details.clone(), account_7, decrease_amount, false, h, current_block, current_block_dupe),
                    (details.clone(), account_8, increase_amount, true, h, current_block, current_block_dupe),
                    (details.clone(), account_9, increase_amount, true, h, current_block, current_block_dupe),
                ];

                if let Err(_) = T::Accounting::handle_multiposting_amounts(forward_keys, reversal_keys) {
                    fail!(Error::<T>::ErrorInAccounting3);
                }

                // export details for final payment steps
                (o, details)
            }
            // This state is not allowed for this functions
            (Unlocked, Locked) => fail!(Error::<T>::ErrorNotAllowed4),
            // Owner has been given permission by beneficiary to release funds
            (Unlocked, Unlocked) => fail!(Error::<T>::ErrorNotAllowed5),
        };

        // Set release lock "buyer who has approved invoice"
        // this may have been set independently, but is required for next step
        if let Err(_) = Self::set_release_state(payer.clone(), Unlocked, h.clone(), uid.clone()) {
            fail!(Error::<T>::ErrorReleaseState);
        }

        // Unlock, tansfer funds and mark hash as settled in full
        if let Err(_) = Self::unlock_funds_for_beneficiary(beneficiary.clone(), h.clone(), uid.clone()) {
            fail!(Error::<T>::ErrorUnlocking);
        }

        Self::deposit_event(Event::InvoiceSettled(uid));

        ok()
    }

    /// check owner (of hash) - if anything fails then returns false
    fn check_ref_owner(o: T::AccountId, h: T::Hash) -> bool {
        match Self::prefunding_hash_owner(&h) {
            Some(owners) if owners.0 == o => true,
            _ => false,
        }
    }

    /// Sets the release state by the owner or the beneficiary is only called when something already exists
    fn set_release_state(o: T::AccountId, o_lock: LockStatus, h: T::Hash, uid: T::Hash) -> DispatchResultWithPostInfo {
        use LockStatus::*;

        // 0= false, 1=true
        // 10, sender can take after deadline (initial state)
        // 11, accepted by recipient. (funds locked, nobody can take)
        // 01, sender approves (recipient can take, or refund)
        // 00, only the recipient authorises sender to retake funds regardless of deadline.
        // Initialise new tuple with some dummy values
        let mut change = (o.clone(), Unlocked, o.clone(), Unlocked);

        match Self::prefunding_hash_owner(&h) {
            Some(state_lock) => {
                let locks = (state_lock.1, state_lock.3);
                change.0 = state_lock.0.clone();
                change.2 = state_lock.2.clone();
                let commander = state_lock.0.clone();
                let fulfiller = state_lock.2.clone();
                match locks {
                    // In this state the commander has created the lock, but it has not been accepted.
                    // The commander can withdraw the lock (set to false) if the deadline has passed, or
                    // the fulfiller can accept the order (set to true)
                    (Locked, Unlocked) => {
                        match o_lock {
                            Locked => {
                                if o == commander {
                                    fail!(Error::<T>::ErrorWrongState1);
                                } else if o == fulfiller {
                                    change.1 = state_lock.1;
                                    change.3 = o_lock;
                                } else {
                                    fail!(Error::<T>::ErrorLockNotAllowed1);
                                };
                            }
                            Unlocked => {
                                // We do care if the deadline has passed IF this is the commander calling directly
                                // but that must be handled outside of this function
                                if o == commander {
                                    change.1 = o_lock;
                                    change.3 = state_lock.3;
                                } else if o == fulfiller {
                                    fail!(Error::<T>::ErrorWrongState2);
                                } else {
                                    fail!(Error::<T>::ErrorLockNotAllowed2);
                                };
                            }
                        }
                    }
                    // In this state the commander can change the lock, and they can only change it to false
                    // In this state the fulfiller can change the lock, and they can only change it to false
                    (Locked, Locked) => match o_lock {
                        Locked => fail!(Error::<T>::ErrorWrongState3),
                        Unlocked => {
                            if o == commander {
                                change.1 = o_lock;
                                change.3 = state_lock.3;
                            } else if o == fulfiller {
                                change.1 = state_lock.1;
                                change.3 = o_lock;
                            } else {
                                fail!(Error::<T>::ErrorLockNotAllowed3);
                            }
                        }
                    },
                    // In this state the commander cannot change the lock
                    // In this state the fulfiller can change the lock, and they can only change it to false
                    (Unlocked, Locked) => match o_lock {
                        Locked => fail!(Error::<T>::ErrorLockNotAllowed4),
                        Unlocked => {
                            if o == commander {
                                fail!(Error::<T>::ErrorWrongState5);
                            } else if o == fulfiller {
                                change.1 = state_lock.1;
                                change.3 = o_lock;
                            } else {
                                fail!(Error::<T>::ErrorLockNotAllowed5);
                            };
                        }
                    },
                    // This state should technically make the funds refundable to the buyer.
                    // Even if the buy wanted to set this state they cannot. Meaning they must create a new order.
                    (Unlocked, Unlocked) => fail!(Error::<T>::ErrorLockNotAllowed5),
                }
            }
            None => fail!(Error::<T>::ErrorHashDoesNotExist2),
        };
        PrefundingHashOwner::<T>::insert(&h, change);
        // Issue event
        Self::deposit_event(Event::PrefundingLockSet(uid));

        ok()
    }

    /// check beneficiary (of hash reference)
    fn check_ref_beneficiary(o: T::AccountId, h: T::Hash) -> bool {
        match Self::prefunding_hash_owner(&h) {
            Some(owners) if owners.2 == o => true,
            _ => false,
        }
    }

    /// unlock for owner
    fn unlock_funds_for_owner(o: T::AccountId, h: T::Hash, _uid: T::Hash) -> DispatchResultWithPostInfo {
        use LockStatus::*;

        if Self::reference_valid(h) == false {
            fail!(Error::<T>::ErrorHashDoesNotExist3);
        }

        if Self::check_ref_owner(o.clone(), h) == false {
            fail!(Error::<T>::ErrorNotOwner2);
        }

        match Self::get_release_state(h) {
            // submitted, but not yet accepted
            // Check if the dealine has passed. If not funds cannot be release
            (Locked, Unlocked) => {
                if Self::prefund_deadline_passed(h) {
                    let status: Status = 50; // Abandoned or Cancelled
                    if let Err(_) = Self::cancel_prefunding_lock(o.clone(), h, status) {
                        fail!(Error::<T>::ErrorCancelFailed2);
                    }
                } else {
                    fail!(Error::<T>::ErrorDeadlineInPlay);
                }
            }
            (Locked, Locked) => fail!(Error::<T>::ErrorFundsInPlay2),
            (Unlocked, Locked) => fail!(Error::<T>::ErrorNotAllowed6),
            (Unlocked, Unlocked) => {
                // Owner has been  given permission by beneficiary to release funds
                let status: Status = 50; // Abandoned or cancelled
                if let Err(_) = Self::cancel_prefunding_lock(o.clone(), h, status) {
                    fail!(Error::<T>::ErrorCancellingPrefund);
                }
            }
        }

        ok()
    }
}
