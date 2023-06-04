//! Vest state and routines
use {crate::math, anchor_lang::prelude::*};

#[account]
#[derive(Default, Debug)]
pub struct Vest {
    // Note: this is the flat amount of token allocated to the vest
    pub amount: u64,
    pub unlock_start_timestamp: i64,
    pub unlock_end_timestamp: i64,

    pub claimed_amount: u64,
    pub last_claim_timestamp: i64,

    pub owner: Pubkey,
    pub bump: u8,
}

impl Vest {
    pub const LEN: usize = 8 + std::mem::size_of::<Vest>();

    // Scale amounts during calculation to increase precision
    pub const CALC_PRECISION_POWER: u128 = 10i64.pow(6u32) as u128;

    pub fn get_claimable_amount(&self, current_time: i64) -> Result<u64> {
        // Nothing claimable
        if self.amount == 0
            || current_time < self.unlock_start_timestamp
            || self.amount == self.claimed_amount
        {
            return Ok(0);
        }

        // Everything remaining is claimable
        if current_time > self.unlock_end_timestamp {
            return Ok(math::checked_sub(self.amount, self.claimed_amount)?);
        }

        let unlock_duration_in_seconds: u128 =
            math::checked_as_u128(self.unlock_end_timestamp - self.unlock_start_timestamp)?;

        let scaled_amount: u128 =
            math::checked_mul(self.amount as u128, Vest::CALC_PRECISION_POWER)?;

        let scaled_amount_claimable_per_second: u128 =
            math::checked_div(scaled_amount, unlock_duration_in_seconds)?;

        let claimable_duration_in_seconds: u128 = math::checked_as_u128({
            if self.last_claim_timestamp == 0 {
                current_time - self.unlock_start_timestamp
            } else {
                current_time - self.last_claim_timestamp
            }
        })?;

        let scaled_claimable_amount: u128 = math::checked_mul(
            claimable_duration_in_seconds,
            scaled_amount_claimable_per_second,
        )?;

        let claimable_amount: u64 = math::checked_as_u64(math::checked_div(
            scaled_claimable_amount,
            Vest::CALC_PRECISION_POWER,
        )?)?;

        Ok(claimable_amount)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_vest_fixture(
        amount: u64,
        unlock_start_timestamp: i64,
        unlock_end_timestamp: i64,
    ) -> Vest {
        Vest {
            amount,
            // Unix timestamps (seconds)
            unlock_start_timestamp,
            unlock_end_timestamp,
            claimed_amount: 0,
            last_claim_timestamp: 0,
            owner: Pubkey::default(),
            bump: 255,
        }
    }

    #[test]
    fn test_get_claimable_amount() {
        // 24h vesting
        let unlock_start_timestamp = 1_600_000_000;
        let unlock_end_timestamp = 1_600_086_400;
        let vest_amount = 10_000;

        // Before the vesting
        {
            // Nothing to claim
            {
                let vest =
                    get_vest_fixture(vest_amount, unlock_start_timestamp, unlock_end_timestamp);
                assert_eq!(0, vest.get_claimable_amount(1_599_990_000).unwrap());
            }
        }

        // In the middle of the vesting
        {
            // Never claimed anything
            {
                let vest =
                    get_vest_fixture(vest_amount, unlock_start_timestamp, unlock_end_timestamp);
                assert_eq!(4_999, vest.get_claimable_amount(1_600_043_200).unwrap());
            }

            // Already claimed 25% of the tokens
            {
                let mut vest =
                    get_vest_fixture(vest_amount, unlock_start_timestamp, unlock_end_timestamp);

                vest.claimed_amount = 2_499;
                vest.last_claim_timestamp = 1_600_021_600;

                assert_eq!(2_499, vest.get_claimable_amount(1_600_043_200).unwrap());
            }

            // Already claimed 50% of the tokens
            {
                let mut vest =
                    get_vest_fixture(vest_amount, unlock_start_timestamp, unlock_end_timestamp);

                vest.claimed_amount = 4_999;
                vest.last_claim_timestamp = 1_600_043_200;

                assert_eq!(0, vest.get_claimable_amount(1_600_043_200).unwrap());
            }
        }

        // After the vesting
        {
            // Never claimed anything
            {
                let vest =
                    get_vest_fixture(vest_amount, unlock_start_timestamp, unlock_end_timestamp);
                assert_eq!(10_000, vest.get_claimable_amount(1_600_090_000).unwrap());
            }

            // Claimed 50% already
            {
                let mut vest =
                    get_vest_fixture(vest_amount, unlock_start_timestamp, unlock_end_timestamp);

                vest.claimed_amount = 4_999;
                vest.last_claim_timestamp = 1_600_043_200;
                assert_eq!(5_001, vest.get_claimable_amount(1_600_090_000).unwrap());
            }

            // Claimed everything
            {
                let mut vest =
                    get_vest_fixture(vest_amount, unlock_start_timestamp, unlock_end_timestamp);

                vest.claimed_amount = 10_000;
                vest.last_claim_timestamp = 1_600_088_000;
                assert_eq!(0, vest.get_claimable_amount(1_600_090_000).unwrap());
            }
        }

        // Special case: nothing in the vest
        {
            let vest = get_vest_fixture(0, unlock_start_timestamp, unlock_end_timestamp);
            assert_eq!(0, vest.get_claimable_amount(1_600_090_000).unwrap());
        }
    }
}
