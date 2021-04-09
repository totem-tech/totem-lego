use super::WithdrawReasons;
// use super::super::misc::WithdrawReasons;

impl WithdrawReasons {
	/// Lock amouts in the escrow.
	pub const ESCROW: WithdrawReasons = WithdrawReasons { bits: 0b00100000 };
}
