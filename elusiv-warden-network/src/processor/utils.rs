use solana_program::{clock::Clock, program_error::ProgramError, sysvar::Sysvar};

pub fn current_timestamp() -> Result<u64, ProgramError> {
    let clock = Clock::get()?;
    Ok(clock.unix_timestamp.try_into().unwrap())
}

pub fn get_day_and_year() -> Result<(u32, u16), ProgramError> {
    let clock = Clock::get()?;
    let timestamp = clock.unix_timestamp.try_into().unwrap();
    unix_timestamp_to_day_and_year(timestamp).ok_or(ProgramError::UnsupportedSysvar)
}

const TWO_K_EPOCH: u64 = 946_684_800;
const TWO_K_100_EPOCH: u64 = 4_102_444_800;
const DAYS_PER_QUADRENNIAL: u64 = 365 * 4 + 1;

/// Returns the day (of the year) and year for a unix-timestamp
///
/// # Notes
///
/// Will return [`None`] for all years outside the range 2000-2099
pub fn unix_timestamp_to_day_and_year(timestamp: u64) -> Option<(u32, u16)> {
    if !(TWO_K_EPOCH..TWO_K_100_EPOCH).contains(&timestamp) {
        return None;
    }

    let days_since_2k = (timestamp - TWO_K_EPOCH) / 86_400;
    let years = days_since_2k / DAYS_PER_QUADRENNIAL * 4;
    let mut days = days_since_2k % DAYS_PER_QUADRENNIAL;
    let mut year = years;

    for y in years..years + 3 {
        let d = 365 + u64::from(y % 4 == 0);
        if days >= d {
            days -= d;
            year += 1;
        } else {
            break;
        }
    }

    Some((days as u32 + 1, 2000 + year as u16))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_unix_timestamp_to_day_and_year() {
        assert_eq!(
            unix_timestamp_to_day_and_year(946684800).unwrap(),
            (1, 2000)
        );
        assert_eq!(
            unix_timestamp_to_day_and_year(978220800).unwrap(),
            (366, 2000)
        );

        assert_eq!(
            unix_timestamp_to_day_and_year(978307200).unwrap(),
            (1, 2001)
        );
        assert_eq!(
            unix_timestamp_to_day_and_year(1009756800).unwrap(),
            (365, 2001)
        );

        assert_eq!(
            unix_timestamp_to_day_and_year(1072915200).unwrap(),
            (1, 2004)
        );
        assert_eq!(
            unix_timestamp_to_day_and_year(1104451200).unwrap(),
            (366, 2004)
        );

        assert_eq!(
            unix_timestamp_to_day_and_year(1671713372).unwrap(),
            (356, 2022)
        );

        let mut timestamp = TWO_K_EPOCH;
        for year in 0..99 {
            for day in 0..365 + u32::from(year % 4 == 0) {
                assert_eq!(
                    unix_timestamp_to_day_and_year(timestamp).unwrap(),
                    (day + 1, year + 2000)
                );
                timestamp += 86_400;
            }
        }
    }
}
