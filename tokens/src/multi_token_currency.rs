use crate::MultiTokenImbalanceWithZeroTrait;
use codec::FullCodec;
use frame_support::traits::{BalanceStatus, ExistenceRequirement, Get, Imbalance, SignedImbalance, WithdrawReasons};
use frame_support::Parameter;
use mangata_primitives::{Balance as BalancePrimitive, TokenId};
use sp_runtime::traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize, Member};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::fmt::Debug;
use sp_std::result;

/// Abstraction over a fungible assets system.
pub trait MultiTokenCurrency<AccountId> {
	/// The balance of an account.
	type Balance: AtLeast32BitUnsigned
		+ FullCodec
		+ Copy
		+ MaybeSerializeDeserialize
		+ Debug
		+ Default
		+ From<BalancePrimitive>
		+ Into<BalancePrimitive>;

	type CurrencyId: Parameter
		+ Member
		+ Copy
		+ MaybeSerializeDeserialize
		+ Ord
		+ Default
		+ AtLeast32BitUnsigned
		+ FullCodec
		+ From<TokenId>
		+ Into<TokenId>;

	/// The opaque token type for an imbalance. This is returned by unbalanced
	/// operations and must be dealt with. It may be dropped but cannot be
	/// cloned.
	type PositiveImbalance: Imbalance<Self::Balance, Opposite = Self::NegativeImbalance>
		+ MultiTokenImbalanceWithZeroTrait<Self::CurrencyId>;

	/// The opaque token type for an imbalance. This is returned by unbalanced
	/// operations and must be dealt with. It may be dropped but cannot be
	/// cloned.
	type NegativeImbalance: Imbalance<Self::Balance, Opposite = Self::PositiveImbalance>
		+ MultiTokenImbalanceWithZeroTrait<Self::CurrencyId>;

	// PUBLIC IMMUTABLES

	/// The combined balance of `who`.
	fn total_balance(currency_id: Self::CurrencyId, who: &AccountId) -> Self::Balance;

	/// Same result as `slash(who, value)` (but without the side-effects)
	/// assuming there are no balance changes in the meantime and only the
	/// reserved balance is not taken into account.
	fn can_slash(currency_id: Self::CurrencyId, who: &AccountId, value: Self::Balance) -> bool;

	/// The total amount of issuance in the system.
	fn total_issuance(currency_id: Self::CurrencyId) -> Self::Balance;

	/// The minimum balance any single account may have. This is equivalent to
	/// the `Balances` module's `ExistentialDeposit`.
	fn minimum_balance(currency_id: Self::CurrencyId) -> Self::Balance;

	/// Reduce the total issuance by `amount` and return the according
	/// imbalance. The imbalance will typically be used to reduce an account by
	/// the same amount with e.g. `settle`.
	///
	/// This is infallible, but doesn't guarantee that the entire `amount` is
	/// burnt, for example in the case of underflow.
	fn burn(currency_id: Self::CurrencyId, amount: Self::Balance) -> Self::PositiveImbalance;

	/// Increase the total issuance by `amount` and return the according
	/// imbalance. The imbalance will typically be used to increase an account
	/// by the same amount with e.g. `resolve_into_existing` or
	/// `resolve_creating`.
	///
	/// This is infallible, but doesn't guarantee that the entire `amount` is
	/// issued, for example in the case of overflow.
	fn issue(acurrency_id: Self::CurrencyId, amount: Self::Balance) -> Self::NegativeImbalance;

	/// Produce a pair of imbalances that cancel each other out exactly.
	///
	/// This is just the same as burning and issuing the same amount and has no
	/// effect on the total issuance.
	fn pair(
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
	) -> (Self::PositiveImbalance, Self::NegativeImbalance) {
		(
			Self::burn(currency_id, amount.clone()),
			Self::issue(currency_id, amount),
		)
	}

	/// The 'free' balance of a given account.
	///
	/// This is the only balance that matters in terms of most operations on
	/// tokens. It alone is used to determine the balance when in the contract
	/// execution environment. When this balance falls below the value of
	/// `ExistentialDeposit`, then the 'current account' is
	/// deleted: specifically `FreeBalance`.
	///
	/// `system::AccountNonce` is also deleted if `ReservedBalance` is also zero
	/// (it also gets collapsed to zero if it ever becomes less than
	/// `ExistentialDeposit`.
	fn free_balance(currency_id: Self::CurrencyId, who: &AccountId) -> Self::Balance;

	/// Returns `Ok` iff the account is able to make a withdrawal of the given
	/// amount for the given reason. Basically, it's just a dry-run of
	/// `withdraw`.
	///
	/// `Err(...)` with the reason why not otherwise.
	fn ensure_can_withdraw(
		currency_id: Self::CurrencyId,
		who: &AccountId,
		_amount: Self::Balance,
		reasons: WithdrawReasons,
		new_balance: Self::Balance,
	) -> DispatchResult;

	// PUBLIC MUTABLES (DANGEROUS)

	/// Transfer some liquid free balance to another staker.
	///
	/// This is a very high-level function. It will ensure all appropriate fees
	/// are paid and no imbalance in the system remains.
	fn transfer(
		currency_id: Self::CurrencyId,
		source: &AccountId,
		dest: &AccountId,
		value: Self::Balance,
		existence_requirement: ExistenceRequirement,
	) -> DispatchResult;

	/// Deducts up to `value` from the combined balance of `who`, preferring to
	/// deduct from the free balance. This function cannot fail.
	///
	/// The resulting imbalance is the first item of the tuple returned.
	///
	/// As much funds up to `value` will be deducted as possible. If this is
	/// less than `value`, then a non-zero second item will be returned.
	fn slash(
		currency_id: Self::CurrencyId,
		who: &AccountId,
		value: Self::Balance,
	) -> (Self::NegativeImbalance, Self::Balance);

	/// Mints `value` to the free balance of `who`.
	///
	/// If `who` doesn't exist, nothing is done and an Err returned.
	fn deposit_into_existing(
		currency_id: Self::CurrencyId,
		who: &AccountId,
		value: Self::Balance,
	) -> result::Result<Self::PositiveImbalance, DispatchError>;

	/// Similar to deposit_creating, only accepts a `NegativeImbalance` and
	/// returns nothing on success.
	fn resolve_into_existing(
		currency_id: Self::CurrencyId,
		who: &AccountId,
		value: Self::NegativeImbalance,
	) -> result::Result<(), Self::NegativeImbalance> {
		let v = value.peek();
		match Self::deposit_into_existing(currency_id, who, v) {
			Ok(opposite) => Ok(drop(value.offset(opposite))),
			_ => Err(value),
		}
	}

	/// Adds up to `value` to the free balance of `who`. If `who` doesn't exist,
	/// it is created.
	///
	/// Infallible.
	fn deposit_creating(
		currency_id: Self::CurrencyId,
		who: &AccountId,
		value: Self::Balance,
	) -> Self::PositiveImbalance;

	/// Similar to deposit_creating, only accepts a `NegativeImbalance` and
	/// returns nothing on success.
	fn resolve_creating(currency_id: Self::CurrencyId, who: &AccountId, value: Self::NegativeImbalance) {
		let v = value.peek();
		drop(value.offset(Self::deposit_creating(currency_id, who, v)));
	}

	/// Removes some free balance from `who` account for `reason` if possible.
	/// If `liveness` is `KeepAlive`, then no less than `ExistentialDeposit`
	/// must be left remaining.
	///
	/// This checks any locks, vesting, and liquidity requirements. If the
	/// removal is not possible, then it returns `Err`.
	///
	/// If the operation is successful, this will return `Ok` with a
	/// `NegativeImbalance` whose value is `value`.
	fn withdraw(
		currency_id: Self::CurrencyId,
		who: &AccountId,
		value: Self::Balance,
		reasons: WithdrawReasons,
		liveness: ExistenceRequirement,
	) -> result::Result<Self::NegativeImbalance, DispatchError>;

	/// Similar to withdraw, only accepts a `PositiveImbalance` and returns
	/// nothing on success.
	fn settle(
		currency_id: Self::CurrencyId,
		who: &AccountId,
		value: Self::PositiveImbalance,
		reasons: WithdrawReasons,
		liveness: ExistenceRequirement,
	) -> result::Result<(), Self::PositiveImbalance> {
		let v = value.peek();
		match Self::withdraw(currency_id, who, v, reasons, liveness) {
			Ok(opposite) => Ok(drop(value.offset(opposite))),
			_ => Err(value),
		}
	}

	/// Ensure an account's free balance equals some value; this will create the
	/// account if needed.
	///
	/// Returns a signed imbalance and status to indicate if the account was
	/// successfully updated or update has led to killing of the account.
	fn make_free_balance_be(
		currency_id: Self::CurrencyId,
		who: &AccountId,
		balance: Self::Balance,
	) -> SignedImbalance<Self::Balance, Self::PositiveImbalance>;
}

/// A currency where funds can be reserved from the user.
pub trait MultiTokenReservableCurrency<AccountId>: MultiTokenCurrency<AccountId> {
	/// Same result as `reserve(who, value)` (but without the side-effects)
	/// assuming there are no balance changes in the meantime.
	fn can_reserve(currency_id: Self::CurrencyId, who: &AccountId, value: Self::Balance) -> bool;

	/// Deducts up to `value` from reserved balance of `who`. This function
	/// cannot fail.
	///
	/// As much funds up to `value` will be deducted as possible. If the reserve
	/// balance of `who` is less than `value`, then a non-zero second item will
	/// be returned.
	fn slash_reserved(
		currency_id: Self::CurrencyId,
		who: &AccountId,
		value: Self::Balance,
	) -> (Self::NegativeImbalance, Self::Balance);

	/// The amount of the balance of a given account that is externally
	/// reserved; this can still get slashed, but gets slashed last of all.
	///
	/// This balance is a 'reserve' balance that other subsystems use in order
	/// to set aside tokens that are still 'owned' by the account holder, but
	/// which are suspendable.
	///
	/// When this balance falls below the value of `ExistentialDeposit`, then
	/// this 'reserve account' is deleted: specifically, `ReservedBalance`.
	///
	/// `system::AccountNonce` is also deleted if `FreeBalance` is also zero (it
	/// also gets collapsed to zero if it ever becomes less than
	/// `ExistentialDeposit`.
	fn reserved_balance(currency_id: Self::CurrencyId, who: &AccountId) -> Self::Balance;

	/// Moves `value` from balance to reserved balance.
	///
	/// If the free balance is lower than `value`, then no funds will be moved
	/// and an `Err` will be returned to notify of this. This is different
	/// behavior than `unreserve`.
	fn reserve(currency_id: Self::CurrencyId, who: &AccountId, value: Self::Balance) -> DispatchResult;

	/// Moves up to `value` from reserved balance to free balance. This function
	/// cannot fail.
	///
	/// As much funds up to `value` will be moved as possible. If the reserve
	/// balance of `who` is less than `value`, then the remaining amount will be
	/// returned.
	///
	/// # NOTES
	///
	/// - This is different from `reserve`.
	/// - If the remaining reserved balance is less than `ExistentialDeposit`,
	///   it will
	/// invoke `on_reserved_too_low` and could reap the account.
	fn unreserve(currency_id: Self::CurrencyId, who: &AccountId, value: Self::Balance) -> Self::Balance;

	/// Moves up to `value` from reserved balance of account `slashed` to
	/// balance of account `beneficiary`. `beneficiary` must exist for this to
	/// succeed. If it does not, `Err` will be returned. Funds will be placed in
	/// either the `free` balance or the `reserved` balance, depending on the
	/// `status`.
	///
	/// As much funds up to `value` will be deducted as possible. If this is
	/// less than `value`, then `Ok(non_zero)` will be returned.
	fn repatriate_reserved(
		currency_id: Self::CurrencyId,
		slashed: &AccountId,
		beneficiary: &AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError>;
}

/// An identifier for a lock. Used for disambiguating different locks so that
/// they can be individually replaced or removed.
pub type LockIdentifier = [u8; 8];

/// A currency whose accounts can have liquidity restrictions.
pub trait MultiTokenLockableCurrency<AccountId>: MultiTokenCurrency<AccountId> {
	/// The quantity used to denote time; usually just a `BlockNumber`.
	type Moment;

	/// The maximum number of locks a user should have on their account.
	type MaxLocks: Get<u32>;

	/// Create a new balance lock on account `who`.
	///
	/// If the new lock is valid (i.e. not already expired), it will push the
	/// struct to the `Locks` vec in storage. Note that you can lock more funds
	/// than a user has.
	///
	/// If the lock `id` already exists, this will update it.
	fn set_lock(
		currency_id: Self::CurrencyId,
		id: LockIdentifier,
		who: &AccountId,
		amount: Self::Balance,
		reasons: WithdrawReasons,
	);

	/// Changes a balance lock (selected by `id`) so that it becomes less liquid
	/// in all parameters or creates a new one if it does not exist.
	///
	/// Calling `extend_lock` on an existing lock `id` differs from `set_lock`
	/// in that it applies the most severe constraints of the two, while
	/// `set_lock` replaces the lock with the new parameters. As in,
	/// `extend_lock` will set:
	/// - maximum `amount`
	/// - bitwise mask of all `reasons`
	fn extend_lock(
		currency_id: Self::CurrencyId,
		id: LockIdentifier,
		who: &AccountId,
		amount: Self::Balance,
		reasons: WithdrawReasons,
	);

	/// Remove an existing lock.
	fn remove_lock(currency_id: Self::CurrencyId, id: LockIdentifier, who: &AccountId);
}

pub trait MultiTokenCurrencyExtended<AccountId>: MultiTokenCurrency<AccountId> {
	fn create(address: &AccountId, amount: Self::Balance) -> Self::CurrencyId;
	fn mint(currency_id: Self::CurrencyId, address: &AccountId, amount: Self::Balance) -> DispatchResult;
	fn get_next_currency_id() -> Self::CurrencyId;
	fn exists(currency_id: Self::CurrencyId) -> bool;
	fn burn_and_settle(currency_id: Self::CurrencyId, who: &AccountId, amount: Self::Balance) -> DispatchResult;
}
