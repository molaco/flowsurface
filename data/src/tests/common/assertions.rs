//! Custom assertion helpers for database testing

use exchange::{Trade, Kline};
use exchange::util::Price;

/// Assert two trades are equal with floating point tolerance
pub fn assert_trade_eq(actual: &Trade, expected: &Trade, context: &str) {
    assert_eq!(actual.time, expected.time, "{}: time mismatch", context);
    assert_eq!(actual.is_sell, expected.is_sell, "{}: is_sell mismatch", context);
    assert!(
        price_approx_equal(actual.price, expected.price),
        "{}: price mismatch - actual: {}, expected: {}",
        context,
        actual.price.to_f32(),
        expected.price.to_f32()
    );
    assert!(
        approx_equal(actual.qty, expected.qty, 0.0001),
        "{}: qty mismatch - actual: {}, expected: {}",
        context,
        actual.qty,
        expected.qty
    );
}

/// Assert two klines are equal with floating point tolerance
pub fn assert_kline_eq(actual: &Kline, expected: &Kline, context: &str) {
    assert_eq!(actual.time, expected.time, "{}: time mismatch", context);
    assert!(
        price_approx_equal(actual.open, expected.open),
        "{}: open mismatch - actual: {}, expected: {}",
        context,
        actual.open.to_f32(),
        expected.open.to_f32()
    );
    assert!(
        price_approx_equal(actual.high, expected.high),
        "{}: high mismatch - actual: {}, expected: {}",
        context,
        actual.high.to_f32(),
        expected.high.to_f32()
    );
    assert!(
        price_approx_equal(actual.low, expected.low),
        "{}: low mismatch - actual: {}, expected: {}",
        context,
        actual.low.to_f32(),
        expected.low.to_f32()
    );
    assert!(
        price_approx_equal(actual.close, expected.close),
        "{}: close mismatch - actual: {}, expected: {}",
        context,
        actual.close.to_f32(),
        expected.close.to_f32()
    );
}

/// Compare two trade vectors for equality
pub fn compare_trade_vectors(actual: &[Trade], expected: &[Trade]) -> Result<(), String> {
    if actual.len() != expected.len() {
        return Err(format!(
            "Trade count mismatch: actual {} vs expected {}",
            actual.len(),
            expected.len()
        ));
    }

    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        if a.time != e.time {
            return Err(format!("Trade {}: time mismatch - {} vs {}", i, a.time, e.time));
        }
        if a.is_sell != e.is_sell {
            return Err(format!("Trade {}: is_sell mismatch", i));
        }
        if !price_approx_equal(a.price, e.price) {
            return Err(format!(
                "Trade {}: price mismatch - {} vs {}",
                i,
                a.price.to_f32(),
                e.price.to_f32()
            ));
        }
        if !approx_equal(a.qty, e.qty, 0.0001) {
            return Err(format!("Trade {}: qty mismatch - {} vs {}", i, a.qty, e.qty));
        }
    }

    Ok(())
}

fn approx_equal(a: f32, b: f32, epsilon: f32) -> bool {
    (a - b).abs() < epsilon
}

fn price_approx_equal(a: Price, b: Price) -> bool {
    approx_equal(a.to_f32(), b.to_f32(), 0.00001)
}
