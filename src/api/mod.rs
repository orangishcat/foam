use std::cmp::min;

use chrono::TimeDelta;

pub mod schoology;
pub mod types;

pub fn exponential_retry(attempts: u32) -> TimeDelta {
    TimeDelta::seconds(2_u32.pow(min(attempts, 10)) as i64)
}
