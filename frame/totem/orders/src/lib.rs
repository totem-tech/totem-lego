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

//! Totem Orders Module
//!
//! # Overview
//!
//! The orders module supports creation of purchase orders and tasks and other types of market order.
//!
//! A basic workflow is as follows:
//!
//! * In general orders are assigned to a partner that the ordering identity already knows and is required to be accepted by that party to become active.
//! * Orders can be made without already knowing the seller - these are called market orders
//! * The order can be prefunded by calling into the prefunding module, which updates the accounting ledgers.
//! * Once the order is accepted, the work must begin, and once completed, the vendor sets the state to completed.
//! * The completion state also generates the invoice, and relevant accounting postings for both the buyer and the seller.
//! * The completed work is then approved by the buyer (or disputed or rejected). An approval triggers the release of prefunds and
//! the invoice is marked as settled in the accounts for both parties
//!
//! The main types used in this module are:
//!
//! * Product = Hash;
//! * UnitPrice = i128; // This does not need a unit of currency because it is allways the internal functional currency
//! * Quantity = u128;
//! * UnitOfMeasure = u16;
//! * buy_or_sell: u16, // 0: buy, 1: sell, extensible
//! * amount: AccountBalanceOf<T>, // amount should be the sum of all the items untiprices * quantities
//! * open_closed: bool, // 0: open(true) 1: closed(false)
//! * order_type: u16, // 0 Services, 1 Goods, 2 Inventory
//! * deadline: u64, // prefunding acceptance deadline
//! * due_date: u64, // due date is the future delivery date (in blocks)

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{dispatch::EncodeLike, fail, pallet_prelude::*};
use frame_system::pallet_prelude::*;

use sp_runtime::traits::Convert;
use sp_std::{prelude::*, vec};

use totem_common::traits::{accounting::Posting, bonsai::Storing, orders::Validating, prefunding::Encumbrance};
use totem_common::{ok, StorageMapExt};

// Totem Config Types
type AccountBalanceOf<T> = <<T as Config>::Accounting as Posting<
    <T as frame_system::Config>::AccountId,
    <T as frame_system::Config>::Hash,
    <T as frame_system::Config>::BlockNumber,
    <T as pallet_balances::Config>::Balance,
>>::LedgerBalance;

// 0=Unlocked(false) 1=Locked(true)
pub type UnLocked<T> = <<T as Config>::Prefunding as Encumbrance<
    <T as frame_system::Config>::AccountId,
    <T as frame_system::Config>::Hash,
    <T as frame_system::Config>::BlockNumber,
>>::LockStatus;

// Module Types
type OrderStatus = u16; // Generic Status for whatever the HashReference refers to

#[repr(u16)]
#[derive(Debug, Decode, Encode, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    Submitted = 0,
    Accepted = 1,
    Rejected = 2,
}
impl EncodeLike<ApprovalStatus> for u16 {}
impl Default for ApprovalStatus {
    fn default() -> Self {
        ApprovalStatus::Submitted
    }
}

/// The order header: contains common values for all items
#[derive(PartialEq, Eq, Copy, Clone, Debug, Encode, Decode, Default)]
pub struct OrderHeader<AccountId> {
    pub commander: AccountId,
    pub fulfiller: AccountId,
    pub approver: AccountId,
    pub order_status: u16,
    pub approval_status: ApprovalStatus,
    pub buy_or_sell: u16,
    pub amount: i128,
    pub market_order: bool,
    pub order_type: u16,
    pub deadline: u32,
    pub due_date: u32,
}

#[derive(PartialEq, Eq, Clone, Debug, Encode, Decode, Default)]
pub struct OrderItem<Hash> {
    pub product: Hash,
    pub unit_price: i128,
    pub quantity: u128,
    pub unit_of_measure: u16,
}

#[derive(PartialEq, Eq, Clone, Debug, Encode, Decode, Default)]
pub struct TXKeysL<Hash> {
    pub record_id: Hash,
    pub parent_id: Hash,
    pub bonsai_token: Hash,
    pub tx_uid: Hash,
}

#[derive(PartialEq, Eq, Clone, Debug, Encode, Decode, Default)]
pub struct TXKeysM<Hash> {
    pub record_id: Hash,
    pub bonsai_token: Hash,
    pub tx_uid: Hash,
}

#[derive(PartialEq, Eq, Clone, Debug, Encode, Decode, Default)]
pub struct TXKeysS<Hash> {
    pub bonsai_token: Hash,
    pub tx_uid: Hash,
}

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    #[pallet::pallet]
    #[pallet::generate_store(trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn owner)]
    pub type Owner<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Vec<T::Hash>>;

    #[pallet::storage]
    #[pallet::getter(fn beneficiary)]
    pub type Beneficiary<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Vec<T::Hash>>;

    #[pallet::storage]
    #[pallet::getter(fn approver)]
    pub type Approver<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Vec<T::Hash>>;

    #[pallet::storage]
    #[pallet::getter(fn postulate)]
    pub type Postulate<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, Vec<T::Hash>>;

    #[pallet::storage]
    #[pallet::getter(fn orders)]
    pub type Orders<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, OrderHeader<T::AccountId>>;

    #[pallet::storage]
    #[pallet::getter(fn order_items)]
    pub type OrderItems<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, Vec<OrderItem<T::Hash>>>;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_accounting::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type OrderConversions: Convert<i128, AccountBalanceOf<Self>>
            + Convert<i128, u128>
            + Convert<bool, UnLocked<Self>>
            + Convert<AccountBalanceOf<Self>, i128>
            + Convert<AccountBalanceOf<Self>, u128>
            + Convert<u32, Self::BlockNumber>
            + Convert<Self::BlockNumber, u32>;
        type Accounting: Posting<Self::AccountId, Self::Hash, Self::BlockNumber, Self::Balance>;
        type Prefunding: Encumbrance<Self::AccountId, Self::Hash, Self::BlockNumber>;
        type Bonsai: Storing<Self::Hash>;
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Cannot change an order that you are not the approver of.
        ErrorNotApprover,
        /// This hash already exists! Try again.
        ErrorHashExists,
        /// This hash does not exit.
        ErrorHashExists2,
        /// This hash does not exit.
        ErrorHashExists3,
        /// Cannot make an order for yourself!
        ErrorCannotBeBoth,
        /// Cannot make an order for yourself!
        ErrorCannotBeBoth2,
        /// You should not be doing this!
        ErrorURNobody,
        /// Order already accepted - cannot change now!
        ErrorOrderStatus1,
        /// Incorrect Order Status!
        ErrorOrderStatus2,
        /// The order has an unkown state!
        ErrorOrderStatus3,
        /// The submitted status not allowed.
        ErrorApprStatus,
        /// Already approved!
        ErrorApproved,
        /// Order status is not allowed!
        ErrorStatusNotAllowed1,
        /// Order already accepted. Order status is not allowed!
        ErrorStatusNotAllowed2,
        /// The order has a status that cannot be changed!
        ErrorStatusNotAllowed3,
        /// The order has an unkown state!
        ErrorStatusNotAllowed4,
        /// The order has an unkown state!
        ErrorStatusNotAllowed5,
        /// This is not your order or wrong status
        ErrorStatusNotAllowed6,
        /// Not allowed to fulfill your own order!
        ErrorFulfiller,
        /// Amount cannot be less than zero!
        ErrorAmount,
        /// Deadline is too short! 48 hours is minimum deadline.
        ErrorShortDeadline,
        /// Due date must be at least 1 hour after deadline
        ErrorShortDueDate,
        /// This situation is not implemented yet: Invoice is disputed
        ErrorNotImplmented1,
        /// Unable to fetch order with this reference
        ErrorGettingOrder,
        /// Error setting prefunding state
        ErrorSetPrefundState,
        /// Error from prefunding module - in check approver
        ErrorInPrefunding1,
        /// Error in Processing Order Acceptance status
        ErrorInPrefunding2,
        /// Error in rejecting order adjusting commander settings
        ErrorInPrefunding3,
        /// Error in rejecting order releasing commander lock
        ErrorInPrefunding4,
        /// Error in prefunding module to send invoice
        ErrorInPrefunding5,
        /// Error in prefunding settling invoice
        ErrorInPrefunding6,
        /// Error setting the first prefunding request
        ErrorInPrefunding7,
        /// Error Cannot make an market order against a parent order
        ErrorMarketOrder,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0/*TODO*/)]
        /// Only the owner of an order can delete it provided no work has been done on it.
        fn delete_order(origin: OriginFor<T>, tx_keys_medium: TXKeysM<T::Hash>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            <T::Bonsai as Storing<T::Hash>>::start_tx(tx_keys_medium.tx_uid.clone())?;

            // Only delete order if it has not been accepted by the fulfiller.
            match Self::orders(&tx_keys_medium.record_id) {
                Some(order) => {
                    // Order is owned by sender, status unaccepted a
                    let approver: T::AccountId = order.approver;
                    let order_status: u16 = order.order_status;
                    if (approver.clone(), order_status) == (who, 0_u16) {
                        Owner::<T>::mutate_(&order.commander, |owner| owner.retain(|v| v != &tx_keys_medium.record_id));
                        Beneficiary::<T>::mutate_(&order.fulfiller, |owner| {
                            owner.retain(|v| v != &tx_keys_medium.record_id)
                        });
                        // <Approver<T>>::mutate(&approver, |owner| {
                        Approver::<T>::mutate_(approver, |owner| owner.retain(|v| v != &tx_keys_medium.record_id));
                        Postulate::<T>::remove(&tx_keys_medium.record_id);
                        Orders::<T>::remove(&tx_keys_medium.record_id);
                        OrderItems::<T>::remove(&tx_keys_medium.record_id);
                    } else {
                        fail!(Error::<T>::ErrorStatusNotAllowed6);
                    }
                }
                // Order does not exist
                None => fail!(Error::<T>::ErrorHashExists3),
            }
            <T::Bonsai as Storing<T::Hash>>::end_tx(tx_keys_medium.tx_uid)?;

            ok()
        }

        #[pallet::weight(0/*TODO*/)]
        /// Creates either a sales order or a purchase order with multi-line items and a parent order
        /// Will be used for the marketplace in order to set up open orders
        fn create_order(
            origin: OriginFor<T>,
            approver: T::AccountId,
            fulfiller: T::AccountId,
            buy_or_sell: u16,
            total_amount: i128,
            market_order: bool,
            order_type: u16,
            deadline: u32,
            due_date: u32,
            order_items: Vec<OrderItem<T::Hash>>,
            tx_keys_large: TXKeysL<T::Hash>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            <T::Bonsai as Storing<T::Hash>>::start_tx(tx_keys_large.tx_uid.clone())?;
            // Check that the supplied record_id does not exist
            if Orders::<T>::contains_key(&tx_keys_large.record_id) {
                fail!(Error::<T>::ErrorHashExists);
            }

            let mut approval_status = ApprovalStatus::Submitted;
            // Check that it is an open order
            if market_order {
                // process open order - ignore fulfiller
                // check that the order does not have a parent - by default the parent and the record_id must be the same
                if tx_keys_large.record_id == tx_keys_large.parent_id {
                } else {
                    fail!(Error::<T>::ErrorMarketOrder);
                }
                // Go further - Store the Order
                ();
            } else {
                // closed order, fulfiller must be completed and it must not be the origin
                if fulfiller == who {
                    fail!(Error::<T>::ErrorCannotBeBoth2);
                }
                // The order may have a parent - by default the parent and the record_id are the same, but they may also be different
                if tx_keys_large.record_id == tx_keys_large.parent_id {
                    // This order has no parent therefore is a simple unfunded order with a known fulfiller
                    // TODO
                    ();
                } else {
                    // This order has a parent therefore it is a proposal and this means there is a fulfiller
                    // check that that the parent hash exists
                    if Orders::<T>::contains_key(&tx_keys_large.parent_id) == false {
                        fail!(Error::<T>::ErrorHashExists2);
                    };
                    // if the approver is also the initiator of the order then automatically approve the order
                    if Self::check_approver(who.clone(), approver.clone(), tx_keys_large.record_id.clone()) {
                        // the order is approved because the approver is the commander.
                        approval_status = ApprovalStatus::Accepted;
                    } else {
                        // the order is not yet approved.
                        // This is NOT an error but requires further processing by the approver.
                        // As this is a proposal against a parent order then associate the child with the parent
                        // This does not happen when it is a simple order
                        Postulate::<T>::mutate_(&tx_keys_large.parent_id, |v| v.push(tx_keys_large.record_id));
                        // <TxList<T>>::mutate(list_key, |tx_list| tx_list.push(u));
                    }
                }
                let order_header: OrderHeader<T::AccountId> = OrderHeader {
                    commander: who.clone(),
                    fulfiller: fulfiller.clone(),
                    approver: who.clone(),
                    order_status: 0u16,
                    approval_status: approval_status,
                    buy_or_sell: buy_or_sell,
                    amount: total_amount,
                    market_order: market_order,
                    order_type: order_type,
                    deadline: deadline,
                    due_date: due_date,
                };
                Self::set_order(who, fulfiller, tx_keys_large.record_id, order_header, order_items)?;
            }
            <T::Bonsai as Storing<T::Hash>>::end_tx(tx_keys_large.tx_uid)?;
            Self::deposit_event(Event::OrderCreated(tx_keys_large.tx_uid.clone(), tx_keys_large.record_id));

            ok()
        }

        #[pallet::weight(0/*TODO*/)]
        /// Create Simple Prefunded Service Order
        /// Can specify an approver. If the approver is the same as the sender then the order is considered approved by default
        fn create_spfso(
            origin: OriginFor<T>,
            approver: T::AccountId,
            fulfiller: T::AccountId,
            buy_or_sell: u16,               // 0: buy, 1: sell, extensible
            total_amount: i128,             // amount should be the sum of all the items untiprices * quantities
            market_order: bool,             // 0: open(false) 1: closed(true)
            order_type: u16,                // 0: service, 1: inventory, 2: asset extensible
            deadline: u32,                  // prefunding acceptance deadline
            due_date: u32,                  // due date is the future delivery date (in blocks)
            order_item: OrderItem<T::Hash>, // for simple items there will only be one item, item number is accessed by its position in Vec
            bonsai_token: T::Hash,          // Bonsai data Hash
            tx_uid: T::Hash,                // Bonsai data Hash
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            <T::Bonsai as Storing<T::Hash>>::start_tx(tx_uid.clone())?;
            // Generate Hash for order
            let order_hash: T::Hash = <<T as Config>::Accounting as Posting<
                T::AccountId,
                T::Hash,
                T::BlockNumber,
                T::Balance,
            >>::get_pseudo_random_hash(who.clone(), approver.clone());
            if Orders::<T>::contains_key(&order_hash) {
                fail!(Error::<T>::ErrorHashExists);
            }
            Self::set_simple_prefunded_service_order(
                who,
                approver,
                fulfiller,
                buy_or_sell,
                total_amount,
                market_order,
                order_type,
                deadline,
                due_date,
                order_hash,
                order_item,
                bonsai_token,
                tx_uid,
            )?;
            <T::Bonsai as Storing<T::Hash>>::end_tx(tx_uid)?;

            Self::deposit_event(Event::OrderCreated(tx_uid, order_hash));

            ok()
        }

        #[pallet::weight(0/*TODO*/)]
        /// Change Simple Prefunded Service Order.
        /// Can only be changed by the original ordering party, and only before it is accepted and the deadline or due date is not passed
        fn change_spfso(
            origin: OriginFor<T>,
            approver: T::AccountId,
            fulfiller: T::AccountId,
            amount: i128,
            deadline: u32,
            due_date: u32,
            order_item: OrderItem<T::Hash>,
            record_id: T::Hash,
            bonsai_token: T::Hash,
            tx_uid: T::Hash,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            // check owner of this record
            <T::Bonsai as Storing<T::Hash>>::start_tx(tx_uid)?;
            Self::change_simple_prefunded_order(
                who.clone(),
                approver.clone(),
                fulfiller.clone(),
                amount,
                deadline,
                due_date,
                order_item,
                record_id,
                bonsai_token,
            )?;
            <T::Bonsai as Storing<T::Hash>>::end_tx(tx_uid)?;

            Self::deposit_event(Event::OrderUpdated(tx_uid));

            ok()
        }

        #[pallet::weight(0/*TODO*/)]
        /// Sets the approval status of an order
        /// Can only be used by the nominated approver (must be known to the ordering party)
        fn change_approval(
            origin: OriginFor<T>,
            h: T::Hash,
            s: ApprovalStatus,
            b: T::Hash,
            tx_uid: T::Hash,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            <T::Bonsai as Storing<T::Hash>>::start_tx(tx_uid)?;
            Self::change_approval_state(who, h, s, b)?;
            <T::Bonsai as Storing<T::Hash>>::end_tx(tx_uid)?;
            Self::deposit_event(Event::InvoiceSettled(h));

            ok()
        }

        #[pallet::weight(0/*TODO*/)]
        /// Can be used by buyer or seller
        /// Buyer - Used by the buyer to accept or reject (TODO) the invoice that was raised by the seller.
        /// Seller - Used to accept, reject or invoice the order.
        fn handle_spfso(
            origin: OriginFor<T>,
            h: T::Hash,
            s: OrderStatus,
            tx_uid: T::Hash,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            <T::Bonsai as Storing<T::Hash>>::start_tx(tx_uid.clone())?;
            // get order details and determine if the sender is the buyer or the seller
            let order_hdr = Self::orders(&h).ok_or(Error::<T>::ErrorGettingOrder)?;
            let commander: T::AccountId = order_hdr.commander.clone();
            let fulfiller: T::AccountId = order_hdr.fulfiller.clone();
            if who == commander {
                // This is the buyer
                //TODO if the order us passed as an arg it doesn't need to be read again
                Self::accept_prefunded_invoice(who.clone(), h.clone(), s, order_hdr.clone(), tx_uid)?;
                Self::deposit_event(Event::InvoiceSettled(tx_uid));
            } else if who == fulfiller {
                // This is the seller
                //TODO if the order us passed as an arg it doesn't need to be read again
                if let Err(_) =
                    Self::set_state_simple_prefunded_closed_order(who.clone(), h.clone(), s, order_hdr.clone(), tx_uid)
                {
                    fail!(Error::<T>::ErrorSetPrefundState);
                }
            } else {
                fail!(Error::<T>::ErrorURNobody)
            }

            <T::Bonsai as Storing<T::Hash>>::end_tx(tx_uid)?;

            ok()
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        OrderCreated(T::Hash, T::Hash),
        OrderUpdated(T::Hash),
        OrderCreatedForApproval(T::Hash),
        OrderCreatedForApproval2(T::Hash),
        OrderStatusUpdate(T::Hash),
        OrderCompleted(T::Hash),
        InvoiceSettled(T::Hash),
    }
}

impl<T: Config> Pallet<T> {
    /// Create Open Order
    /// This function simply stores an open sales or purchase order. It is intended for the marketplace,
    /// yet it can be a complex purchase or sales order
    /// The order can have another party as an approver or not
    /// * The order is not underwritten by prefunding
    /// * Because this is creation it cannot have a parent
    /// The approver should be able to set the status, and once approved the process should continue further
    /// pending_approval (0), approved(1), rejected(2) are the tree states to be set
    /// If the status is 2 the commander may edit and resubmit
    fn check_approver(c: T::AccountId, a: T::AccountId, h: T::Hash) -> bool {
        // If the approver is the same as the commander then it is approved by default & update accordingly
        // If the approver is not the commander, then update but also set the status to pending approval.
        // You should gracefully exit after this function call in this case.
        let approved = c == a;

        Approver::<T>::mutate_(&a, |approver| approver.push(h.clone()));

        approved
    }

    /// API Open an order for a specific AccountId and prefund it. This is equivalent to an encumbrance.
    /// The amount is the functional currency and conversions are not necessary at this stage of accounting.
    /// The UI therefore handles presentation or reporting currency translations at spot rate
    /// This is not for goods.
    /// If the order is open, the the fulfiller is ignored.
    /// Order type is generally goods (0) or services (1) but is left open for future-proofing
    fn set_simple_prefunded_service_order(
        commander: T::AccountId,
        approver: T::AccountId,
        fulfiller: T::AccountId,
        buy_or_sell: u16,   // 0: buy, 1: sell, extensible
        amount: i128,       // amount should be the sum of all the items untiprices * quantities
        market_order: bool, // 0: open(false) 1: closed(true)
        order_type: u16,    // 0: personal, 1: business, extensible
        deadline: u32,      // prefunding acceptance deadline
        due_date: u32,      // due date is the future delivery date (in blocks)
        order_hash: T::Hash,
        order_item: OrderItem<T::Hash>, // for simple items there will only be one item, item number is accessed by its position in Vec
        bonsai_token: T::Hash,
        uid: T::Hash,
    ) -> DispatchResultWithPostInfo {
        // Set order status to submitted by default
        // submitted(0), accepted(1), rejected(2), disputed(3), blocked(4), invoiced(5),
        let order_status: OrderStatus = 0;
        let mut fulfiller_override: T::AccountId = fulfiller.clone();

        // TODO Rewrite this MARKET_ORDER reversing the bool. This is because the API open_closed will be replaced by market_order bool.
        // let mut market_order: bool = false;
        match market_order {
            true => {
                // market_order = true;
                fulfiller_override = commander.clone();
            }
            // This is an open order. No need to check the fulfiller, but will override with the commander for time being.
            false => {
                // this is a closed order, still will need to check or set the approver status
                // if fulfiller is the commander throw error
                if commander == fulfiller {
                    fail!(Error::<T>::ErrorCannotBeBoth);
                }
            }
        }

        // check or set the approver status
        if Self::check_approver(commander.clone(), approver.clone(), order_hash.clone()) {
            // the order is approved.
            let approval_status: ApprovalStatus = ApprovalStatus::Accepted;
            let deadline_converted: T::BlockNumber =
                <T::OrderConversions as Convert<u32, T::BlockNumber>>::convert(deadline);
            // approval status has been set to approved, continue.
            // Set prefunding first. It does not matter if later the process fails, as this is locking funds for the commander
            // The risk is that they cannot get back the funds until after the deadline, even of they want to cancel.
            let balance_amount: u128 = <T::OrderConversions as Convert<i128, u128>>::convert(amount.clone());

            if let Err(_) = Self::set_prefunding(
                commander.clone(),
                fulfiller.clone(),
                balance_amount,
                deadline_converted,
                order_hash.clone(),
                uid,
            ) {
                // Error from setting prefunding "somewhere" ;)
                fail!(Error::<T>::ErrorInPrefunding1);
            }
            let order_header: OrderHeader<T::AccountId> = OrderHeader {
                commander: commander.clone(),
                fulfiller: fulfiller_override.clone(),
                approver: approver,
                order_status: order_status,
                approval_status: approval_status,
                buy_or_sell: buy_or_sell,
                amount: amount,
                market_order: market_order,
                order_type: order_type,
                deadline: deadline,
                due_date: due_date,
            };
            let vec_order_items = vec![order_item.clone()];
            Self::set_order(commander, fulfiller, order_hash.clone(), order_header, vec_order_items)?;
        } else {
            // the order is not yet approved.
            // This is NOT an error but requires further processing by the approver. Exiting gracefully.
            Self::deposit_event(Event::OrderCreatedForApproval(uid));
        }

        // claim hash in Bonsai
        <T::Bonsai as Storing<T::Hash>>::claim_data(order_hash.clone(), bonsai_token.clone())?;

        ok()
    }

    /// Calls the prefunding module to lock funds. This does not perform an update or lock release
    fn set_prefunding(
        c: T::AccountId,
        f: T::AccountId,
        a: u128,
        d: T::BlockNumber,
        o: T::Hash,
        u: T::Hash,
    ) -> DispatchResultWithPostInfo {
        if let Err(_) = T::Prefunding::prefunding_for(c, f, a, d, o, u) {
            fail!(Error::<T>::ErrorInPrefunding7);
        }

        ok()
    }

    /// Stores the order data and sets the order status.
    fn set_order(
        c: T::AccountId,
        f: T::AccountId,
        o: T::Hash,
        h: OrderHeader<T::AccountId>,
        i: Vec<OrderItem<T::Hash>>,
    ) -> DispatchResultWithPostInfo {
        // Set hash for commander
        Owner::<T>::mutate_(&c, |owner| owner.push(o.clone()));
        // This will be a market order if the fulfiller is the same as the commander
        // In this case do not set the beneficiary storage
        if c != f {
            // Set hash for fulfiller
            Beneficiary::<T>::mutate_(&f, |beneficiary| beneficiary.push(o.clone()));
        }
        // Set details of Order
        Orders::<T>::insert(&o, h);
        OrderItems::<T>::insert(&o, i);

        ok()
    }

    /// API This function is used to accept or reject the order by the named approver. Mainly used for the API
    fn change_approval_state(a: T::AccountId, h: T::Hash, s: ApprovalStatus, b: T::Hash) -> DispatchResultWithPostInfo {
        // is the supplied account the approver of the hash supplied?
        let mut order_hdr: OrderHeader<T::AccountId> = Self::orders(&h).ok_or("some error")?;
        if a == order_hdr.approver && order_hdr.order_status == 0 {
            match order_hdr.order_status {
                0 | 2 => {
                    // can only change to approved (1)
                    match s {
                        ApprovalStatus::Accepted => (),
                        _ => fail!(Error::<T>::ErrorApprStatus),
                    }
                }
                1 => {
                    // Can only change to 0 or 2
                    match s {
                        ApprovalStatus::Submitted | ApprovalStatus::Rejected => (),
                        _ => fail!(Error::<T>::ErrorApprStatus),
                    }
                }
                _ => fail!(Error::<T>::ErrorApprStatus),
            }
            // All tests passed, set status to whatever.
            order_hdr.order_status = s as u16;
            Orders::<T>::insert(&h, order_hdr);
        } else {
            fail!(Error::<T>::ErrorNotApprover);
        }

        Self::deposit_event(Event::OrderStatusUpdate(b));

        ok()
    }

    /// API Allows commander to change the order either before it is accepted by beneficiary, or
    /// when it has been rejected by approver
    fn change_simple_prefunded_order(
        commander: T::AccountId,
        approver: T::AccountId,
        fulfiller: T::AccountId,
        amount: i128,
        deadline: u32,
        due_date: u32,
        order_item: OrderItem<T::Hash>,
        reference: T::Hash,
        bonsai_token: T::Hash,
    ) -> DispatchResultWithPostInfo {
        // Check that the hash exist
        // let order_hdr: OrderHeader<T::AccountId> = Self:order_header(&reference).ok_or("some error")?;
        let order_hdr: OrderHeader<T::AccountId> = Self::orders(&reference).ok_or("some error")?;
        // check that the Order state is 0 or 2 (submitted or rejected)
        // check that the approval is 0 or 2 pending approval or rejected
        match order_hdr.order_status {
            0 | 2 => {
                match order_hdr.approval_status {
                    ApprovalStatus::Submitted | ApprovalStatus::Rejected => (), // submitted pending approval or rejected
                    ApprovalStatus::Accepted => fail!(Error::<T>::ErrorApproved),
                }
            }
            1 => fail!(Error::<T>::ErrorOrderStatus1),
            _ => fail!(Error::<T>::ErrorOrderStatus2),
        };
        // check that at least one of these has changed:
        // let mut dl: u64;
        // let mut dd: u64;
        let current_block = frame_system::Module::<T>::block_number();
        // apply a new fulfiller but check that it isn't the commander
        if order_hdr.commander == commander {
            fail!(Error::<T>::ErrorFulfiller);
        }
        if order_hdr.amount != amount {
            if amount < 0i128 {
                fail!(Error::<T>::ErrorAmount);
            }

            // IMPORTANT TODO
            // Check that the amount is the sum of all the items
        }

        let current_block_converted: u32 =
            <T::OrderConversions as Convert<T::BlockNumber, u32>>::convert(current_block);
        if order_hdr.deadline != deadline {
            // TODO This may be unusable/unworkable needs trying out
            // 48 hours is the minimum deadline
            // every time there is a change the deadline gets pushed back by 48 hours byond the current block
            let min_deadline = current_block_converted + 11520_u32;
            if deadline < min_deadline {
                fail!(Error::<T>::ErrorShortDeadline);
            }
            // dl = deadline;
        }
        if order_hdr.due_date != due_date {
            // due date must be at least 1 hours after deadline (TODO - Validate! as this is a guess)
            // This is basically adding 49 hours to the current block
            let minimum_due_date = current_block_converted + 11760_u32;
            if due_date < minimum_due_date {
                fail!(Error::<T>::ErrorShortDueDate);
            }
            // dd = due_date;
        }
        // Create Order sub header
        let order_header: OrderHeader<T::AccountId> = OrderHeader {
            commander: commander.clone(),
            fulfiller: fulfiller.clone(),
            approver: approver.clone(),
            order_status: 0,
            approval_status: order_hdr.approval_status,
            buy_or_sell: order_hdr.buy_or_sell,
            amount: amount,
            market_order: order_hdr.market_order,
            order_type: order_hdr.order_type,
            deadline: deadline,
            due_date: due_date,
        };
        // currently just places all the items in the storage WITHOUT CHECKING
        // TODO check for changes and confirm that amount = sum of all amounts
        let vec_order_items = vec![order_item];
        Self::set_order(order_hdr.commander, fulfiller, reference.clone(), order_header, vec_order_items)?;
        // prefunding can only be cancelled if deadline has passed, otherwise the prefunding remains as a deposit
        // TODO we could use the cancel prefunding function to do this.
        // change hash in Bonsai
        <T::Bonsai as Storing<T::Hash>>::claim_data(reference.clone(), bonsai_token.clone())?;

        ok()
    }

    /// Used by the beneficiary (fulfiller) to accept, reject or invoice the order.
    /// It effectively creates a state change for the order and the prefunding
    /// When accepting, the order is locked for the beneficiary or when rejected the funds are released for the order owner.
    /// When invoicing the
    fn set_state_simple_prefunded_closed_order(
        f: T::AccountId,
        h: T::Hash,
        s: OrderStatus,
        mut order: OrderHeader<T::AccountId>,
        uid: T::Hash,
    ) -> DispatchResultWithPostInfo {
        match order.order_status {
            0 => {
                // Order not accepted yet. Update the status in this module
                match s {
                    1 => {
                        // Order Accepted
                        // Update the prefunding status (confirm locked funds)
                        let lock: UnLocked<T> = <T::OrderConversions as Convert<bool, UnLocked<T>>>::convert(true);
                        match <T::Prefunding as Encumbrance<T::AccountId, T::Hash, T::BlockNumber>>::set_release_state(
                            f, lock, h, uid,
                        ) {
                            Ok(_) => (),
                            Err(_e) => fail!(Error::<T>::ErrorInPrefunding2),
                        }
                    }
                    2 => {
                        // order rejected
                        let lock: UnLocked<T> = <T::OrderConversions as Convert<bool, UnLocked<T>>>::convert(false);
                        // We do not need to set release state for releasing funds for fulfiller.
                        // set release state for releasing funds for commander.
                        match <T::Prefunding as Encumbrance<T::AccountId, T::Hash, T::BlockNumber>>::set_release_state(
                            order.commander.clone(),
                            lock,
                            h,
                            uid.clone(),
                        ) {
                            Ok(_) => (),
                            Err(_e) => {
                                fail!(Error::<T>::ErrorInPrefunding3);
                            }
                        }
                        // now release the funds lock
                        match <T::Prefunding as Encumbrance<T::AccountId,T::Hash,T::BlockNumber>>::unlock_funds_for_owner(order.commander.clone(),h, uid.clone()) {
                            Ok(_) => (),
                            Err(_e) => {
                                fail!(Error::<T>::ErrorInPrefunding4);
                            },
                        }
                    }
                    _ => fail!(Error::<T>::ErrorStatusNotAllowed1),
                }
            }
            // Order already in accepted state - Update the status
            1 => {
                match s {
                    5 => {
                        // Order Completed. Now we are going to issue the invoice.
                        match <T::Prefunding as Encumbrance<T::AccountId, T::Hash, T::BlockNumber>>::send_simple_invoice(
                            f.clone(),
                            order.commander.clone(),
                            order.amount,
                            h,
                            uid,
                        ) {
                            Ok(_) => (),
                            Err(_e) => fail!(Error::<T>::ErrorInPrefunding5),
                        }
                    }
                    _ => fail!(Error::<T>::ErrorStatusNotAllowed2),
                }
            }
            2 | 5 => fail!(Error::<T>::ErrorStatusNotAllowed3),
            _ => fail!(Error::<T>::ErrorStatusNotAllowed4),
        }
        order.order_status = s;
        Orders::<T>::remove(&h);
        Orders::<T>::insert(&h, order);

        Self::deposit_event(Event::OrderCompleted(uid));

        ok()
    }

    /// Used by the buyer to accept or reject (TODO) the invoice that was raised by the seller.
    fn accept_prefunded_invoice(
        o: T::AccountId,
        h: T::Hash,
        s: OrderStatus,
        mut order: OrderHeader<T::AccountId>,
        uid: T::Hash,
    ) -> DispatchResultWithPostInfo {
        // check that this is the fulfiller
        if order.order_status != 5 {
            fail!(Error::<T>::ErrorOrderStatus3)
        }

        // Order has been invoiced. The buyer is now deciding to accept or other
        match s {
            // Invoice is disputed. TODO provide the ability to change the invoice and resubmit
            3 => fail!(Error::<T>::ErrorNotImplmented1),
            6 => {
                // Invoice Accepted. Now pay-up!.
                match <T::Prefunding as Encumbrance<T::AccountId, T::Hash, T::BlockNumber>>::settle_prefunded_invoice(
                    o.clone(),
                    h,
                    uid,
                ) {
                    Ok(_) => (),
                    Err(_e) => {
                        fail!(Error::<T>::ErrorInPrefunding6);
                    }
                }
                Self::deposit_event(Event::InvoiceSettled(uid));
            }
            _ => fail!(Error::<T>::ErrorStatusNotAllowed5),
        }
        // Update the status in this module

        order.order_status = s;
        Orders::<T>::remove(&h);
        Orders::<T>::insert(&h, order);

        ok()
    }

    /// This is used by any party that wants to accept a market order in whole or part.
    /// This is non-blocking and can accept many applicants
    fn postulate_simple_prefunded_open_order() -> DispatchResultWithPostInfo {
        fail!("TODO")
    }
}

pub use pallet::*;

impl<T: Config> Validating<T::AccountId, T::Hash> for Pallet<T> {
    /// Check that the order is somehow managed by this identity. Mainly used for BONSAI
    fn is_order_party(o: T::AccountId, r: T::Hash) -> bool {
        match Self::orders(r) {
            Some(order) if o == order.commander || o == order.fulfiller || o == order.approver => true,
            _ => false,
        }
    }
}
