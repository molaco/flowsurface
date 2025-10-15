use iced::{
    widget::{column, container, row, rule, scrollable, space, text},
    Alignment, Element, Length, Task,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DatabaseManager {
    db_manager: Option<Arc<data::db::DatabaseManager>>,
    stats: Option<DatabaseStats>,
    loading: bool,
}

#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_trades: i64,
    pub total_klines: i64,
    pub total_tickers: i64,
    pub total_exchanges: i64,
    pub database_size_mb: f64,
    pub ticker_breakdown: Vec<TickerStats>,
}

#[derive(Debug, Clone)]
pub struct TickerStats {
    pub symbol: String,
    pub exchange: String,
    pub trade_count: i64,
    pub kline_count: i64,
    pub first_trade: Option<String>,
    pub last_trade: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
    StatsLoaded(Result<DatabaseStats, String>),
}

pub enum Action {
    // Future: actions that need to propagate up to main
}

impl DatabaseManager {
    pub fn new(db_manager: Option<Arc<data::db::DatabaseManager>>) -> Self {
        Self {
            db_manager,
            stats: None,
            loading: false,
        }
    }

    fn format_number(n: i64) -> String {
        let s = n.to_string();
        let mut result = String::new();
        let mut count = 0;
        for c in s.chars().rev() {
            if count == 3 {
                result.push(',');
                count = 0;
            }
            result.push(c);
            count += 1;
        }
        result.chars().rev().collect()
    }

    pub fn set_db_manager(&mut self, db_manager: Option<Arc<data::db::DatabaseManager>>) {
        self.db_manager = db_manager;
    }

    pub fn update(&mut self, message: Message) -> (Task<Message>, Option<Action>) {
        match message {
            Message::Refresh => {
                if let Some(db_manager) = self.db_manager.clone() {
                    self.loading = true;

                    let task = Task::perform(
                        async move { Self::fetch_stats(db_manager).await },
                        Message::StatsLoaded,
                    );

                    return (task, None);
                }
            }
            Message::StatsLoaded(result) => {
                self.loading = false;

                match result {
                    Ok(stats) => {
                        self.stats = Some(stats);
                    }
                    Err(e) => {
                        log::error!("Failed to load database stats: {}", e);
                    }
                }
            }
        }

        (Task::none(), None)
    }

    async fn fetch_stats(
        db_manager: Arc<data::db::DatabaseManager>,
    ) -> Result<DatabaseStats, String> {
        // This runs in background task - won't block UI

        // Query database statistics
        let stats_result = db_manager.with_conn(|conn| {
            use data::db::DatabaseError;

            // Total counts
            let total_trades: i64 = conn
                .query_row("SELECT COUNT(*) FROM trades", [], |row| row.get(0))
                .map_err(|e| DatabaseError::Query(format!("Query failed: {}", e)))?;

            let total_klines: i64 = conn
                .query_row("SELECT COUNT(*) FROM klines", [], |row| row.get(0))
                .map_err(|e| DatabaseError::Query(format!("Query failed: {}", e)))?;

            let total_tickers: i64 = conn
                .query_row("SELECT COUNT(*) FROM tickers", [], |row| row.get(0))
                .map_err(|e| DatabaseError::Query(format!("Query failed: {}", e)))?;

            let total_exchanges: i64 = conn
                .query_row("SELECT COUNT(*) FROM exchanges", [], |row| row.get(0))
                .map_err(|e| DatabaseError::Query(format!("Query failed: {}", e)))?;

            // Database size (approximate from block stats)
            let (total_blocks, block_size): (i64, i64) = conn
                .query_row(
                    "SELECT total_blocks, block_size FROM pragma_database_size()",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap_or((0, 0));

            let database_size_mb = (total_blocks * block_size) as f64 / 1024.0 / 1024.0;

            // Per-ticker breakdown
            let mut ticker_stmt = conn
                .prepare(
                    "SELECT
                        t.symbol,
                        e.name as exchange,
                        COALESCE((SELECT COUNT(*) FROM trades tr WHERE tr.ticker_id = t.ticker_id), 0) as trade_count,
                        COALESCE((SELECT COUNT(*) FROM klines k WHERE k.ticker_id = t.ticker_id), 0) as kline_count,
                        (SELECT MIN(timestamp) FROM trades tr WHERE tr.ticker_id = t.ticker_id) as first_trade_ts,
                        (SELECT MAX(timestamp) FROM trades tr WHERE tr.ticker_id = t.ticker_id) as last_trade_ts
                     FROM tickers t
                     JOIN exchanges e ON t.exchange_id = e.exchange_id
                     ORDER BY trade_count DESC",
                )
                .map_err(|e| DatabaseError::Query(format!("Failed to prepare query: {}", e)))?;

            let ticker_breakdown: Vec<TickerStats> = ticker_stmt
                .query_map([], |row| {
                    let first_trade_ts: Option<i64> = row.get(4)?;
                    let last_trade_ts: Option<i64> = row.get(5)?;

                    Ok(TickerStats {
                        symbol: row.get(0)?,
                        exchange: row.get(1)?,
                        trade_count: row.get(2)?,
                        kline_count: row.get(3)?,
                        first_trade: first_trade_ts.and_then(|ts| {
                            chrono::DateTime::from_timestamp_millis(ts)
                                .map(|dt| dt.to_rfc3339())
                        }),
                        last_trade: last_trade_ts.and_then(|ts| {
                            chrono::DateTime::from_timestamp_millis(ts)
                                .map(|dt| dt.to_rfc3339())
                        }),
                    })
                })
                .map_err(|e| DatabaseError::Query(format!("Query failed: {}", e)))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| DatabaseError::Query(format!("Failed to collect results: {}", e)))?;

            Ok(DatabaseStats {
                total_trades,
                total_klines,
                total_tickers,
                total_exchanges,
                database_size_mb,
                ticker_breakdown,
            })
        });

        stats_result.map_err(|e: data::db::DatabaseError| e.to_string())
    }

    pub fn view(&self) -> Element<Message> {
        let title = text("Database Manager").size(20).width(Length::Fill);

        let refresh_button = iced::widget::button("Refresh").on_press(Message::Refresh);

        let header = row![title, refresh_button]
            .spacing(10)
            .align_y(Alignment::Center);

        let content = if self.loading {
            column![text("Loading statistics...").size(16)]
                .spacing(10)
                .padding(20)
        } else if let Some(stats) = &self.stats {
            self.stats_view(stats)
        } else {
            column![
                text("No database statistics available").size(16),
                text("Click Refresh to load statistics").size(14),
            ]
            .spacing(10)
            .padding(20)
        };

        let main_content = column![
            header,
            rule::horizontal(1.0),
            scrollable(content),
        ]
        .spacing(10)
        .padding(20)
        .width(Length::Fixed(600.0))
        .height(Length::Fixed(700.0));

        container(main_content)
            .style(|theme: &iced::Theme| container::Style {
                background: Some(theme.palette().background.into()),
                border: iced::Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                ..container::Style::default()
            })
            .into()
    }

    fn stats_view(&self, stats: &DatabaseStats) -> iced::widget::Column<Message> {
        // Overview section
        let overview = column![
            text("📊 Overview").size(18),
            row![
                text("Database Size:").width(Length::Fixed(150.0)),
                text(format!("{:.2} MB", stats.database_size_mb)),
            ]
            .spacing(10),
            row![
                text("Total Trades:").width(Length::Fixed(150.0)),
                text(Self::format_number(stats.total_trades)),
            ]
            .spacing(10),
            row![
                text("Total Klines:").width(Length::Fixed(150.0)),
                text(Self::format_number(stats.total_klines)),
            ]
            .spacing(10),
            row![
                text("Tickers:").width(Length::Fixed(150.0)),
                text(format!("{}", stats.total_tickers)),
            ]
            .spacing(10),
            row![
                text("Exchanges:").width(Length::Fixed(150.0)),
                text(format!("{}", stats.total_exchanges)),
            ]
            .spacing(10),
        ]
        .spacing(8)
        .padding(15);

        // Per-ticker breakdown
        let mut ticker_rows = column![text("📁 Data by Ticker").size(18),].spacing(8).padding(15);

        if stats.ticker_breakdown.is_empty() {
            ticker_rows = ticker_rows.push(text("No tickers found").size(14));
        } else {
            for ticker in &stats.ticker_breakdown {
                let range_text = if let (Some(first), Some(last)) = (&ticker.first_trade, &ticker.last_trade) {
                    format!(
                        "  Range: {} to {}",
                        first.split('.').next().unwrap_or(first),
                        last.split('.').next().unwrap_or(last)
                    )
                } else {
                    "  No data".to_string()
                };

                let ticker_row = column![
                    row![text(format!("{} ({})", ticker.symbol, ticker.exchange))
                        .size(16)
                        .width(Length::Fill),],
                    row![
                        text(format!("  Trades: {}", Self::format_number(ticker.trade_count)))
                            .size(14)
                            .width(Length::Fixed(200.0)),
                        text(format!("Klines: {}", Self::format_number(ticker.kline_count))).size(14),
                    ]
                    .spacing(20),
                    row![text(range_text).size(12)],
                ]
                .spacing(4);

                ticker_rows = ticker_rows.push(ticker_row);
                ticker_rows = ticker_rows.push(space::Space::new().height(Length::Fixed(10.0)));
            }
        }

        column![
            overview,
            rule::horizontal(1.0),
            ticker_rows,
        ]
        .spacing(15)
    }
}
