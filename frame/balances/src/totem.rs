// Custom totem stuff.

use super::*;
use frame_support::traits::WithdrawReasons;
use totem_common::traits::accounting::Posting;

/// A currency whose accounts can have liquidity restrictions.
pub trait TotemLockableCurrency<AccountId>: Currency<AccountId> {
    /// The quantity used to denote time; usually just a `BlockNumber`.
    type Moment;

    /// The maximum number of locks a user should have on their account.
    type MaxLocks: Get<u32>;

    /// Create a new balance lock on account `who`.
    ///
    /// If the new lock is valid (i.e. not already expired), it will push the struct to
    /// the `Locks` vec in storage. Note that you can lock more funds than a user has.
    ///
    /// If the lock `id` already exists, this will update it.
    fn totem_set_lock(
        id: LockIdentifier,
        who: &AccountId,
        amount: Self::Balance,
        until: Self::Moment,
        reasons: WithdrawReasons,
    );

    /// Changes a balance lock (selected by `id`) so that it becomes less liquid in all
    /// parameters or creates a new one if it does not exist.
    ///
    /// Calling `extend_lock` on an existing lock `id` differs from `set_lock` in that it
    /// applies the most severe constraints of the two, while `set_lock` replaces the lock
    /// with the new parameters. As in, `extend_lock` will set:
    /// - maximum `amount`
    /// - bitwise mask of all `reasons`
    fn totem_extend_lock(
        id: LockIdentifier,
        who: &AccountId,
        amount: Self::Balance,
        until: Self::Moment,
        reasons: WithdrawReasons,
    );

    /// Remove an existing lock.
    fn totem_remove_lock(id: LockIdentifier, who: &AccountId);
}

impl<T: Config<I>, I: 'static> TotemLockableCurrency<T::AccountId> for Pallet<T, I>
where
    T::Balance: MaybeSerializeDeserialize + Debug,
{
    type Moment = T::BlockNumber;

    type MaxLocks = T::MaxLocks;

    // Set a lock on the balance of `who`.
    // Is a no-op if lock amount is zero or `reasons` `is_none()`.
    fn totem_set_lock(
        id: LockIdentifier,
        who: &T::AccountId,
        amount: T::Balance,
        until: T::BlockNumber,
        reasons: WithdrawReasons,
    ) {
        if amount.is_zero() || reasons.is_empty() {
            return;
        }

        let now = frame_system::Pallet::<T>::block_number();

        // Add or update the new lock
        let mut new_lock = Some(TotemBalanceLock {
            id,
            amount,
            reasons: reasons.into(),
            until,
        });
        let mut locks = Self::totem_locks(who)
            .into_iter()
            .filter_map(|l| {
                if l.id == id {
                    // Update lock
                    new_lock.take()
                } else if l.until < now {
                    // Deadline has passed
                    None
                } else {
                    Some(l)
                }
            })
            .collect::<Vec<_>>();
        if let Some(lock) = new_lock {
            locks.push(lock)
        }

        // Now apply the lock by transfering to the escrow
        if let Ok(_) = Self::transfer_to_the_escrow(who, amount) {
            Self::totem_update_locks(who, &locks[..]);
        }
    }

    // Extend a lock on the balance of `who`.
    // Is a no-op if lock amount is zero or `reasons` `is_none()`.
    fn totem_extend_lock(
        id: LockIdentifier,
        who: &T::AccountId,
        amount: T::Balance,
        until: T::BlockNumber,
        reasons: WithdrawReasons,
    ) {
        if amount.is_zero() || reasons.is_empty() {
            return;
        }

        let now = frame_system::Pallet::<T>::block_number();

        let mut new_lock = Some(TotemBalanceLock {
            id,
            amount,
            reasons: reasons.into(),
            until,
        });
        let mut locks = Self::totem_locks(who)
            .into_iter()
            .filter_map(|l| {
                if l.id == id {
                    new_lock.take().map(|nl| TotemBalanceLock {
                        id: l.id,
                        amount: l.amount.max(nl.amount),
                        reasons: l.reasons | nl.reasons,
                        until: nl.until,
                    })
                } else if l.until < now {
                    // Deadline has passed
                    None
                } else {
                    Some(l)
                }
            })
            .collect::<Vec<_>>();
        if let Some(lock) = new_lock {
            locks.push(lock)
        }

        // Now apply the lock by transfering to the escrow
        if let Ok(_) = Self::transfer_to_the_escrow(who, amount) {
            Self::totem_update_locks(who, &locks[..]);
        }
    }

    fn totem_remove_lock(id: LockIdentifier, who: &T::AccountId) {
        let mut locks = Self::totem_locks(who);

        let mut i = 0;
        while i != locks.len() {
            if locks[i].id == id {
                let l = locks.remove(i);
                Self::make_free_balance_be(who, l.amount);
            } else {
                i += 1;
            }
        }

        Self::totem_update_locks(who, &locks[..]);
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    /// Update the account entry for `who`, given the locks.
    fn totem_update_locks(who: &T::AccountId, locks: &[TotemBalanceLock<T::Balance, T::BlockNumber>]) {
        if locks.len() as u32 > T::MaxLocks::get() {
            log::warn!(
                target: "runtime::balances",
                "Warning: A user has more currency totem locks than expected. \
                A runtime configuration adjustment may be needed."
            );
        }

        let existed = TotemLocks::<T, I>::contains_key(who);
        if locks.is_empty() {
            Locks::<T, I>::remove(who);
            if existed {
                // TODO: use Locks::<T, I>::hashed_key
                // https://github.com/paritytech/substrate/issues/4969
                system::Pallet::<T>::dec_consumers(who);
            }
        } else {
            TotemLocks::<T, I>::insert(who, locks);
            if !existed {
                if system::Pallet::<T>::inc_consumers(who).is_err() {
                    // No providers for the locks. This is impossible under normal circumstances
                    // since the funds that are under the lock will themselves be stored in the
                    // account and therefore will need a reference.
                    log::warn!(
                        target: "runtime::balances",
                        "Warning: Attempt to introduce lock consumer reference, yet no providers. \
                        This is unexpected but should be safe."
                    );
                }
            }
        }
    }

    fn transfer_to_the_escrow(who: &T::AccountId, amount: T::Balance) -> result::Result<(), DispatchError> {
        let imba = Self::withdraw(who, amount, WithdrawReasons::ESCROW, ExistenceRequirement::KeepAlive)?;

        let escrow_account: T::AccountId = T::Accounting::get_escrow_account();

        Self::make_free_balance_be(&escrow_account, amount);
        let _imba_resolved = imba;

        Ok(())
    }
}
