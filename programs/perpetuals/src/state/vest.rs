//! Vest state and routines

use anchor_lang::prelude::*;

use {super::perpetuals::Perpetuals, crate::math};

#[account]
#[derive(Default, Debug)]
pub struct Vest {
    pub amount: u64,
    // unlock_share have implied BPS_DECIMALS decimals
    pub unlock_share: u32,
    pub owner: Pubkey,

    pub bump: u8,
    pub inception_time: i64,
}

/// Cortex
impl Vest {
    pub const LEN: usize = 8 + std::mem::size_of::<Vest>();

    fn get_amount_to_share(&self, circulating_supply: u64) -> Result<u64> {
        math::checked_as_u64(math::checked_div(
            math::checked_mul(self.amount as u128, Perpetuals::BPS_POWER)?,
            circulating_supply as u128,
        )?)
    }
}

#[cfg(test)]
mod test {
    use {super::*, proptest::prelude::*};

    fn get_vest_fixture(amount: u64, unlock_share: u32) -> Vest {
        Vest {
            amount,
            unlock_share,
            owner: Pubkey::default(),
            bump: 255,
            inception_time: 1,
        }
    }

    fn test_get_amount_to_share() {
        proptest!(|(amount: u64, unlock_share: u32)| {
        }
    }
}
