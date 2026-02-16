use crate::types::EarningsStats;
use alloy_primitives::U256;

pub fn calc_earnings_stats(now: i64, payments: &[(i64, U256)]) -> EarningsStats {
    let earnings_stats = EarningsStats::default();
    let mut periods = vec![
        (
            period_windows(now, earnings_stats.period_1.length_hours.into()),
            earnings_stats.period_1,
        ),
        (
            period_windows(now, earnings_stats.period_2.length_hours.into()),
            earnings_stats.period_2,
        ),
        (
            period_windows(now, earnings_stats.period_3.length_hours.into()),
            earnings_stats.period_3,
        ),
        (
            period_windows(now, earnings_stats.period_4.length_hours.into()),
            earnings_stats.period_4,
        ),
    ];

    for (((start, end), (prev_start, prev_end)), period_stats) in &mut periods {
        let amts = payments_in_window(payments, *start, *end);
        let amts_prev = payments_in_window(payments, *prev_start, *prev_end);
        for amt in &amts {
            if *amt > period_stats.largest_payment {
                period_stats.largest_payment = *amt;
            }
            period_stats.total_earned += *amt;
        }
        period_stats.num_payments = amts.len();
        if period_stats.num_payments > 0 {
            period_stats.average_payment = period_stats
                .total_earned
                .checked_div(U256::from(period_stats.num_payments))
                .unwrap_or_default();
            let half = period_stats.num_payments / 2;
            period_stats.median_payment = if period_stats.num_payments % 2 == 0 {
                (amts[half - 1] + amts[half])
                    .checked_div(U256::from(2))
                    .unwrap_or_default()
            } else {
                amts[half]
            }
        };
        let (change_percent, change_amount) =
            calc_change(period_stats.total_earned, amts_prev.iter().sum());
        period_stats.change_percent = change_percent;
        period_stats.change_amount = change_amount;
    }

    EarningsStats {
        period_1: periods[0].1.clone(),
        period_2: periods[1].1.clone(),
        period_3: periods[2].1.clone(),
        period_4: periods[3].1.clone(),
    }
}

fn payments_in_window(payments: &[(i64, U256)], start: i64, end: i64) -> Vec<U256> {
    let mut sorted = payments
        .iter()
        .filter(|(ts, amt)| *ts > start && *ts <= end && *amt > U256::ZERO)
        .map(|(_, amt)| *amt)
        .collect::<Vec<U256>>();
    sorted.sort();
    sorted
}

fn calc_change(current: U256, previous: U256) -> (Option<f64>, f64) {
    let prev = f64::from(previous);
    let amt = f64::from(current) - prev;
    if prev > 0.0 {
        let pct = (amt / prev) * 100.0;
        (Some(pct), amt)
    } else {
        (None, amt)
    }
}

fn period_windows(now: i64, period_hours: i64) -> ((i64, i64), (i64, i64)) {
    let period_secs = period_hours * 3600;
    let end = now;
    let start = now - period_secs + 1;
    let prev_end = start - 1;
    let prev_start = prev_end - period_secs + 1;
    ((start, end), (prev_start, prev_end))
}
