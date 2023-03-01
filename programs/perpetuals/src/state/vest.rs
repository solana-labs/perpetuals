//! Vest state and routines

use anchor_lang::prelude::*;

use {super::perpetuals::Perpetuals, crate::math};

#[account]
#[derive(Default, Debug)]
pub struct Vest {
    // Note: this is the flat amount of token the vest will provide the owner at unlock
    pub amount: u64,
    // Note: the vested amount will unlock when it becomes "unlock_share"% of the circulatin supply
    // unlock_share have implied BPS_DECIMALS decimals
    pub unlock_share: u64,
    pub owner: Pubkey,

    pub bump: u8,
    pub inception_time: i64,
}

/// Cortex
impl Vest {
    pub const LEN: usize = 8 + std::mem::size_of::<Vest>();

    pub fn is_claimable(&self, circulating_supply: u64) -> Result<bool> {
        let amount_share = math::checked_as_u64(math::checked_div(
            math::checked_mul(self.amount as u128, Perpetuals::BPS_POWER)?,
            circulating_supply as u128,
        )?)?;
        Ok(amount_share >= self.unlock_share as u64)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_vest_fixture(amount: u64, unlock_share: u64) -> Vest {
        Vest {
            amount,
            unlock_share,
            owner: Pubkey::default(),
            bump: 255,
            inception_time: 1,
        }
    }

    fn scale_f64(amount: f64, decimals: u8) -> u64 {
        math::checked_as_u64(
            math::checked_float_mul(amount, 10u64.pow(decimals as u32) as f64).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn test_is_claimable() {
        // 1% owned, 1% unlock, OK
        let owner_vest_amount = 1;
        let unlock_percentage = 0.01;
        let circulating_supply = 100;
        let vest = get_vest_fixture(
            owner_vest_amount,
            scale_f64(unlock_percentage, Perpetuals::BPS_DECIMALS),
        );
        assert_eq!(vest.is_claimable(circulating_supply).unwrap(), true);

        // 10% owned, 1% unlock, OK
        let owner_vest_amount = 10;
        let unlock_percentage = 0.01;
        let circulating_supply = 100;
        let vest = get_vest_fixture(
            owner_vest_amount,
            scale_f64(unlock_percentage, Perpetuals::BPS_DECIMALS),
        );
        assert_eq!(vest.is_claimable(circulating_supply).unwrap(), true);

        // 1% owned, 10% unlock, KO
        let owner_vest_amount = 1;
        let unlock_percentage = 0.1;
        let circulating_supply = 100;
        let vest = get_vest_fixture(
            owner_vest_amount,
            scale_f64(unlock_percentage, Perpetuals::BPS_DECIMALS),
        );
        assert_eq!(vest.is_claimable(circulating_supply).unwrap(), false);

        // 0% owned, 1% unlock, KO
        let owner_vest_amount = 0;
        let unlock_percentage = 0.01;
        let circulating_supply = 100;
        let vest = get_vest_fixture(
            owner_vest_amount,
            scale_f64(unlock_percentage, Perpetuals::BPS_DECIMALS),
        );
        assert_eq!(vest.is_claimable(circulating_supply).unwrap(), false);

        // 4.99% owned, 5% unlock, KO
        let owner_vest_amount = 499;
        let unlock_percentage = 0.05;
        let circulating_supply = 10000;
        let vest = get_vest_fixture(
            owner_vest_amount,
            scale_f64(unlock_percentage, Perpetuals::BPS_DECIMALS),
        );
        assert_eq!(vest.is_claimable(circulating_supply).unwrap(), false);
    }
}
