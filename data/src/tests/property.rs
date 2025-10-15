//! Property-based roundtrip tests using proptest

use proptest::prelude::*;
use crate::db::{DatabaseManager, TradesCRUD, DatabaseConfig};
use exchange::{Trade, TickerInfo, Ticker};
use exchange::util::{Price, MinTicksize};
use exchange::adapter::Exchange;

use super::common::setup_test_environment;

// Strategy for generating valid timestamps (2020-2030 range)
fn timestamp_strategy() -> impl Strategy<Value = u64> {
    (1577836800000u64..1893456000000u64) // 2020-01-01 to 2030-01-01 in ms
}

// Strategy for generating valid prices (1.0 to 100000.0)
fn price_strategy() -> impl Strategy<Value = f32> {
    (1.0f32..100000.0f32)
}

// Strategy for generating valid quantities (0.001 to 1000.0)
fn qty_strategy() -> impl Strategy<Value = f32> {
    (0.001f32..1000.0f32)
}

// Strategy for generating Trade instances
fn trade_strategy() -> impl Strategy<Value = Trade> {
    (
        timestamp_strategy(),
        any::<bool>(),
        price_strategy(),
        qty_strategy(),
    )
        .prop_map(|(time, is_sell, price, qty)| Trade {
            time,
            is_sell,
            price: Price::from_f32(price).round_to_min_tick(MinTicksize::from(0.01)),
            qty,
        })
}

proptest! {
    #[test]
    fn test_trade_roundtrip(trade in trade_strategy()) {
        let env = setup_test_environment().expect("Failed to setup test environment");
        let db = env.db();

        let ticker = Ticker::new("TESTUSDT", Exchange::BinanceLinear);
        let ticker_info = TickerInfo::new(ticker, 0.01, 0.001, None);

        // Insert trade
        db.insert_trades(&ticker_info, &[trade])
            .expect("Failed to insert trade");

        // Query it back
        let queried = db
            .query_trades(&ticker_info, trade.time - 1000, trade.time + 1000)
            .expect("Failed to query trade");

        // Verify roundtrip
        prop_assert_eq!(queried.len(), 1, "Expected exactly one trade");

        let queried_trade = &queried[0];
        prop_assert_eq!(queried_trade.time, trade.time, "Time mismatch");
        prop_assert_eq!(queried_trade.is_sell, trade.is_sell, "is_sell mismatch");

        // Allow small floating point differences
        let price_diff = (queried_trade.price.to_f32() - trade.price.to_f32()).abs();
        prop_assert!(price_diff < 0.01, "Price difference too large: {}", price_diff);

        let qty_diff = (queried_trade.qty - trade.qty).abs();
        prop_assert!(qty_diff < 0.0001, "Qty difference too large: {}", qty_diff);
    }

    #[test]
    fn test_price_precision_preservation(price in price_strategy()) {
        // Test that price conversion to database and back preserves precision
        let original_price = Price::from_f32(price).round_to_min_tick(MinTicksize::from(0.01));

        let env = setup_test_environment().expect("Failed to setup test environment");
        let db = env.db();

        let ticker = Ticker::new("TESTUSDT", Exchange::BinanceLinear);
        let ticker_info = TickerInfo::new(ticker, 0.01, 0.001, None);

        let trade = Trade {
            time: 1000000000,
            is_sell: false,
            price: original_price,
            qty: 1.0,
        };

        db.insert_trades(&ticker_info, &[trade])
            .expect("Failed to insert trade");

        let queried = db
            .query_trades(&ticker_info, 999999000, 1000001000)
            .expect("Failed to query trade");

        prop_assert_eq!(queried.len(), 1);

        // Precision should be preserved within 8 decimal places
        let retrieved_price = queried[0].price.to_f32();
        let diff = (retrieved_price - original_price.to_f32()).abs();
        prop_assert!(diff < 0.00000001, "Price precision loss: {}", diff);
    }

    #[test]
    fn test_multiple_trades_order_preservation(
        trades in prop::collection::vec(trade_strategy(), 10..100)
    ) {
        let env = setup_test_environment().expect("Failed to setup test environment");
        let db = env.db();

        let ticker = Ticker::new("TESTUSDT", Exchange::BinanceLinear);
        let ticker_info = TickerInfo::new(ticker, 0.01, 0.001, None);

        // Sort trades by time for insertion
        let mut sorted_trades = trades.clone();
        sorted_trades.sort_by_key(|t| t.time);

        db.insert_trades(&ticker_info, &sorted_trades)
            .expect("Failed to insert trades");

        // Query all trades
        if let Some(first) = sorted_trades.first() {
            if let Some(last) = sorted_trades.last() {
                let queried = db
                    .query_trades(&ticker_info, first.time - 1000, last.time + 1000)
                    .expect("Failed to query trades");

                // Verify order is preserved (trades should be sorted by time)
                for i in 1..queried.len() {
                    prop_assert!(
                        queried[i].time >= queried[i - 1].time,
                        "Trades not sorted by time"
                    );
                }
            }
        }
    }
}
