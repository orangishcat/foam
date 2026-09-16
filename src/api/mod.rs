use chrono::TimeDelta;

pub mod schoology;
pub mod types;

pub fn exponential_retry(attempts: u32) -> TimeDelta {
    TimeDelta::seconds(2_u32.pow(attempts) as i64)
}
