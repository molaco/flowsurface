use crate::chart::{
    Caches, Message, ViewState,
    indicator::{
        indicator_row,
        kline::KlineIndicatorImpl,
        plot::{PlotTooltip, line::LinePlot},
    },
};

use data::chart::{PlotData, kline::KlineDataPoint};
use data::util::format_with_commas;
use exchange::{Kline, Trade};
use exchange::util::Price;

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Theme};

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

pub struct MovingAverageIndicator {
    cache: Caches,
    data: BTreeMap<u64, f32>,     // MA values for rendering
    closes: BTreeMap<u64, f32>,   // Source close prices
    period: usize,
}

impl MovingAverageIndicator {
    pub fn new() -> Self {
        Self {
            cache: Caches::default(),
            data: BTreeMap::new(),
            closes: BTreeMap::new(),
            period: 20,  // Default 20-period SMA
        }
    }

    fn indicator_elem<'a>(
        &'a self,
        main_chart: &'a ViewState,
        visible_range: RangeInclusive<u64>,
    ) -> iced::Element<'a, Message> {
        let period = self.period;
        let tooltip = move |value: &f32, _next: Option<&f32>| {
            PlotTooltip::new(format!("MA ({}): {}", period, format_with_commas(*value)))
        };

        let value_fn = |v: &f32| *v;

        let plot = LinePlot::new(value_fn)
            .stroke_width(1.5)
            .show_points(false)
            .padding(0.05)
            .with_tooltip(tooltip);

        indicator_row(main_chart, &self.cache, plot, &self.data, visible_range)
    }
}

impl KlineIndicatorImpl for MovingAverageIndicator {
    fn clear_all_caches(&mut self) {
        self.cache.clear_all();
    }

    fn clear_crosshair_caches(&mut self) {
        self.cache.clear_crosshair();
    }

    fn element<'a>(
        &'a self,
        chart: &'a ViewState,
        visible_range: RangeInclusive<u64>,
    ) -> iced::Element<'a, Message> {
        self.indicator_elem(chart, visible_range)
    }

    fn rebuild_from_source(&mut self, source: &PlotData<KlineDataPoint>) {
        self.data.clear();
        self.closes.clear();

        match source {
            PlotData::TimeBased(timeseries) => {
                // Collect (timestamp, close) pairs
                let close_vec: Vec<(u64, f32)> = timeseries
                    .datapoints
                    .iter()
                    .map(|(time, dp)| (*time, dp.kline.close.to_f32()))
                    .collect();

                // Store all closes
                for (time, close) in &close_vec {
                    self.closes.insert(*time, *close);
                }

                // Calculate MA using windows
                if close_vec.len() >= self.period {
                    for i in (self.period - 1)..close_vec.len() {
                        let sum: f32 = close_vec[i - (self.period - 1)..=i]
                            .iter()
                            .map(|(_, c)| c)
                            .sum();
                        let ma = sum / self.period as f32;
                        self.data.insert(close_vec[i].0, ma);
                    }
                }
            }
            PlotData::TickBased(tickseries) => {
                // Collect closes
                let closes: Vec<f32> = tickseries
                    .datapoints
                    .iter()
                    .map(|dp| dp.kline.close.to_f32())
                    .collect();

                // Store all closes
                for (idx, close) in closes.iter().enumerate() {
                    self.closes.insert(idx as u64, *close);
                }

                // Calculate MA using windows
                if closes.len() >= self.period {
                    for (i, window) in closes.windows(self.period).enumerate() {
                        let sum: f32 = window.iter().sum();
                        let ma = sum / self.period as f32;
                        let idx = (i + self.period - 1) as u64;
                        self.data.insert(idx, ma);
                    }
                }
            }
        }
        self.clear_all_caches();
    }

    fn on_insert_klines(&mut self, klines: &[Kline]) {
        for kline in klines {
            // Store close
            self.closes.insert(kline.time, kline.close.to_f32());

            // Get last N closes for MA calculation
            let window: Vec<f32> = self.closes
                .range(..=kline.time)
                .rev()
                .take(self.period)
                .map(|(_, &price)| price)
                .collect();

            if window.len() == self.period {
                let sum: f32 = window.iter().sum();
                let ma = sum / self.period as f32;
                self.data.insert(kline.time, ma);
            }
        }
        self.clear_all_caches();
    }

    fn on_insert_trades(
        &mut self,
        _trades: &[Trade],
        old_dp_len: usize,
        source: &PlotData<KlineDataPoint>,
    ) {
        match source {
            PlotData::TimeBased(_) => return,
            PlotData::TickBased(tickseries) => {
                let start_idx = old_dp_len.saturating_sub(1);
                for (idx, dp) in tickseries.datapoints.iter().enumerate().skip(start_idx) {
                    let close = dp.kline.close.to_f32();
                    self.closes.insert(idx as u64, close);

                    // Get last N closes for MA calculation
                    let window: Vec<f32> = self.closes
                        .range(..=(idx as u64))
                        .rev()
                        .take(self.period)
                        .map(|(_, &price)| price)
                        .collect();

                    if window.len() == self.period {
                        let sum: f32 = window.iter().sum();
                        let ma = sum / self.period as f32;
                        self.data.insert(idx as u64, ma);
                    }
                }
            }
        }
        self.clear_all_caches();
    }

    fn on_ticksize_change(&mut self, source: &PlotData<KlineDataPoint>) {
        self.rebuild_from_source(source);
    }

    fn on_basis_change(&mut self, source: &PlotData<KlineDataPoint>) {
        self.rebuild_from_source(source);
    }

    fn draw_overlay(
        &self,
        frame: &mut canvas::Frame,
        chart: &ViewState,
        visible_range: RangeInclusive<u64>,
        theme: &Theme,
    ) -> bool {
        if self.data.is_empty() {
            return true;
        }

        let palette = theme.extended_palette();

        // Helper functions for coordinate conversion
        let interval_to_x = |interval: u64| chart.interval_to_x(interval);
        let price_to_y = |price: Price| chart.price_to_y(price);

        // Collect visible MA points
        let points: Vec<Point> = self.data
            .range(visible_range)
            .map(|(time, value)| {
                let x = interval_to_x(*time);
                let y = price_to_y(Price::from_f32(*value));
                Point::new(x, y)
            })
            .collect();

        // Draw the MA line if we have at least 2 points
        if points.len() >= 2 {
            let mut path_builder = canvas::path::Builder::new();

            // Move to first point
            path_builder.move_to(points[0]);

            // Draw lines to subsequent points
            for point in points.iter().skip(1) {
                path_builder.line_to(*point);
            }

            let path = path_builder.build();

            // Use a nice blue color for MA line
            let stroke = Stroke {
                width: 2.0,
                line_cap: canvas::LineCap::Round,
                line_join: canvas::LineJoin::Round,
                ..Default::default()
            };

            frame.stroke(&path, Stroke::with_color(stroke, palette.primary.base.color));
        }

        true  // This is an overlay indicator
    }

    fn is_overlay_only(&self) -> bool {
        true  // MA is overlay-only, no separate panel
    }
}
