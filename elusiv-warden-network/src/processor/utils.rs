use chrono::{DateTime, Utc, NaiveDateTime, Datelike};
use solana_program::{clock::Clock, program_error::ProgramError, sysvar::Sysvar};

pub fn current_timestamp() -> Result<u64, ProgramError> {
    let clock = Clock::get()?;
    Ok(clock.unix_timestamp.try_into().unwrap())
}

pub fn get_day_and_year() -> Result<(u32, u16), ProgramError> {
    let clock = Clock::get()?;
    let datetime: DateTime<Utc> = DateTime::from_utc(
        NaiveDateTime::from_timestamp(clock.unix_timestamp, 0),
        Utc,
    );
    let year = datetime.year().try_into().unwrap();

    Ok((datetime.day(), year))
}