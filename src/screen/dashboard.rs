pub mod pane;
pub mod panel;
pub mod sidebar;
pub mod tickers_table;

pub use sidebar::Sidebar;

use super::DashboardError;
use crate::{
    chart,
    modal::{self, pane::settings::study::StudyMessage},
    style,
    widget::toast::Toast,
    window::{self, Window},
};
use data::{UserTimezone, chart::Basis, layout::WindowSpec};
use exchange::{
    Kline, PushFrequency, TickMultiplier, TickerInfo, Timeframe, Trade,
    adapter::{
        self, AdapterError, Exchange, PersistStreamKind, ResolvedStream, StreamConfig, StreamKind,
        StreamTicksize, UniqueStreams, aster, binance, bybit, hyperliquid, okex,
    },
    depth::Depth,
    fetcher::{FetchRange, FetchedData},
};

use iced::{
    Element, Length, Subscription, Task, Vector,
    task::{Straw, sipper},
    widget::{
        PaneGrid, center, container,
        pane_grid::{self, Configuration},
    },
};
use iced_futures::futures::TryFutureExt;
use std::{collections::HashMap, path::PathBuf, time::Instant, vec};

#[derive(Debug, Clone)]
pub enum Message {
    Pane(window::Id, pane::Message),
    ChangePaneStatus(uuid::Uuid, pane::Status),
    SavePopoutSpecs(HashMap<window::Id, WindowSpec>),
    ErrorOccurred(Option<uuid::Uuid>, DashboardError),
    Notification(Toast),
    DistributeFetchedData {
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        stream: StreamKind,
        data: FetchedData,
    },
    ResolveStreams(uuid::Uuid, Vec<PersistStreamKind>),
}

pub struct Dashboard {
    pub panes: pane_grid::State<pane::State>,
    pub focus: Option<(window::Id, pane_grid::Pane)>,
    pub popout: HashMap<window::Id, (pane_grid::State<pane::State>, WindowSpec)>,
    pub streams: UniqueStreams,
    layout_id: uuid::Uuid,
    db_manager: Option<std::sync::Arc<data::db::DatabaseManager>>,
}

impl Default for Dashboard {
    fn default() -> Self {
        Self {
            panes: pane_grid::State::with_configuration(Self::default_pane_config()),
            focus: None,
            streams: UniqueStreams::default(),
            popout: HashMap::new(),
            layout_id: uuid::Uuid::new_v4(),
            db_manager: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Notification(Toast),
    DistributeFetchedData {
        layout_id: uuid::Uuid,
        pane_id: uuid::Uuid,
        data: FetchedData,
        stream: StreamKind,
    },
    ResolveStreams {
        pane_id: uuid::Uuid,
        streams: Vec<PersistStreamKind>,
    },
}

impl Dashboard {
    fn default_pane_config() -> Configuration<pane::State> {
        Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: 0.8,
            a: Box::new(Configuration::Split {
                axis: pane_grid::Axis::Horizontal,
                ratio: 0.4,
                a: Box::new(Configuration::Split {
                    axis: pane_grid::Axis::Vertical,
                    ratio: 0.5,
                    a: Box::new(Configuration::Pane(pane::State::default())),
                    b: Box::new(Configuration::Pane(pane::State::default())),
                }),
                b: Box::new(Configuration::Split {
                    axis: pane_grid::Axis::Vertical,
                    ratio: 0.5,
                    a: Box::new(Configuration::Pane(pane::State::default())),
                    b: Box::new(Configuration::Pane(pane::State::default())),
                }),
            }),
            b: Box::new(Configuration::Pane(pane::State::default())),
        }
    }

    pub fn from_config(
        panes: Configuration<pane::State>,
        popout_windows: Vec<(Configuration<pane::State>, WindowSpec)>,
        layout_id: uuid::Uuid,
        db_manager: Option<std::sync::Arc<data::db::DatabaseManager>>,
    ) -> Self {
        let panes = pane_grid::State::with_configuration(panes);

        let mut popout = HashMap::new();

        for (pane, specs) in popout_windows {
            popout.insert(
                window::Id::unique(),
                (pane_grid::State::with_configuration(pane), specs),
            );
        }

        Self {
            panes,
            focus: None,
            streams: UniqueStreams::default(),
            popout,
            layout_id,
            db_manager,
        }
    }

    pub fn load_layout(&mut self, main_window: window::Id) -> Task<Message> {
        let mut open_popouts_tasks: Vec<Task<Message>> = vec![];
        let mut new_popout = Vec::new();
        let mut keys_to_remove = Vec::new();

        for (old_window_id, (_, specs)) in &self.popout {
            keys_to_remove.push((*old_window_id, *specs));
        }

        // remove keys and open new windows
        for (old_window_id, window_spec) in keys_to_remove {
            let (window, task) = window::open(window::Settings {
                position: window::Position::Specific(window_spec.position()),
                size: window_spec.size(),
                exit_on_close_request: false,
                ..window::settings()
            });

            open_popouts_tasks.push(task.then(|_| Task::none()));

            if let Some((removed_pane, specs)) = self.popout.remove(&old_window_id) {
                new_popout.push((window, (removed_pane, specs)));
            }
        }

        // assign new windows to old panes
        for (window, (pane, specs)) in new_popout {
            self.popout.insert(window, (pane, specs));
        }

        Task::batch(open_popouts_tasks).chain(self.refresh_streams(main_window))
    }

    pub fn update(
        &mut self,
        message: Message,
        main_window: &Window,
        layout_id: &uuid::Uuid,
    ) -> (Task<Message>, Option<Event>) {
        match message {
            Message::SavePopoutSpecs(specs) => {
                for (window_id, new_spec) in specs {
                    if let Some((_, spec)) = self.popout.get_mut(&window_id) {
                        *spec = new_spec;
                    }
                }
            }
            Message::ErrorOccurred(pane_id, err) => match pane_id {
                Some(id) => {
                    if let Some(state) = self.get_mut_pane_state_by_uuid(main_window.id, id) {
                        state.status = pane::Status::Ready;
                        state.notifications.push(Toast::error(err.to_string()));
                    }
                }
                _ => {
                    return (
                        Task::done(Message::Notification(Toast::error(err.to_string()))),
                        None,
                    );
                }
            },
            Message::Pane(window, message) => match message {
                pane::Message::PaneClicked(pane) => {
                    self.focus = Some((window, pane));
                }
                pane::Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                    self.panes.resize(split, ratio);
                }
                pane::Message::PaneDragged(event) => {
                    if let pane_grid::DragEvent::Dropped { pane, target } = event {
                        self.panes.drop(pane, target);
                    }
                }
                pane::Message::SplitPane(axis, pane) => {
                    let focus_pane = if let Some((new_pane, _)) =
                        self.panes.split(axis, pane, pane::State::new())
                    {
                        Some(new_pane)
                    } else {
                        None
                    };

                    if Some(focus_pane).is_some() {
                        self.focus = Some((window, focus_pane.unwrap()));
                    }
                }
                pane::Message::ClosePane(pane) => {
                    if let Some((_, sibling)) = self.panes.close(pane) {
                        self.focus = Some((window, sibling));
                    }
                }
                pane::Message::MaximizePane(pane) => {
                    self.panes.maximize(pane);
                }
                pane::Message::Restore => {
                    self.panes.restore();
                }
                pane::Message::ReplacePane(pane) => {
                    if let Some(pane) = self.panes.get_mut(pane) {
                        *pane = pane::State::new();
                    }

                    return (self.refresh_streams(main_window.id), None);
                }
                pane::Message::ShowModal(pane, requested_modal) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        match &state.modal {
                            Some(modal) if modal == &requested_modal => {
                                state.modal = None;
                            }
                            _ => {
                                state.modal = Some(requested_modal);
                            }
                        }
                    }
                }
                pane::Message::HideModal(pane) => {
                    if let Some(pane_state) = self.get_mut_pane(main_window.id, window, pane) {
                        pane_state.modal = None;
                    }
                }
                pane::Message::ChartInteraction(pane, msg) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        match state.content {
                            pane::Content::Heatmap { ref mut chart, .. } => {
                                let Some(chart) = chart else {
                                    panic!(
                                        "chart wasn't initialized when handling chart interaction"
                                    );
                                };
                                chart::update(chart, &msg);
                            }
                            pane::Content::Kline { ref mut chart, .. } => {
                                let Some(chart) = chart else {
                                    panic!(
                                        "chart wasn't initialized when handling chart interaction"
                                    );
                                };
                                chart::update(chart, &msg);
                            }
                            _ => {}
                        }
                    }
                }
                pane::Message::PanelInteraction(pane, msg) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        match state.content {
                            pane::Content::Ladder(Some(ref mut panel)) => {
                                panel::update(panel, msg);
                            }
                            pane::Content::TimeAndSales(Some(ref mut panel)) => {
                                panel::update(panel, msg);
                            }
                            _ => {}
                        }
                    }
                }
                pane::Message::VisualConfigChanged(pane, cfg, to_sync) => {
                    if to_sync {
                        if let Some(state) = self.get_pane(main_window.id, window, pane) {
                            let studies_cfg = state.content.studies();
                            let clusters_cfg = match &state.content {
                                pane::Content::Kline {
                                    kind: data::chart::KlineChartKind::Footprint { clusters, .. },
                                    ..
                                } => Some(*clusters),
                                _ => None,
                            };

                            self.iter_all_panes_mut(main_window.id)
                                .for_each(|(_, _, state)| {
                                    let should_apply = match state.settings.visual_config {
                                        Some(ref current_cfg) => {
                                            std::mem::discriminant(current_cfg)
                                                == std::mem::discriminant(&cfg)
                                        }
                                        None => matches!(
                                            (&cfg, &state.content),
                                            (
                                                data::chart::VisualConfig::Kline(_),
                                                pane::Content::Kline { .. }
                                            ) | (
                                                data::chart::VisualConfig::Heatmap(_),
                                                pane::Content::Heatmap { .. }
                                            ) | (
                                                data::chart::VisualConfig::TimeAndSales(_),
                                                pane::Content::TimeAndSales(_)
                                            )
                                        ),
                                    };

                                    if should_apply {
                                        state.settings.visual_config = Some(cfg);
                                        state.content.change_visual_config(cfg);

                                        if let Some(studies) = &studies_cfg {
                                            state.content.update_studies(studies.clone());
                                        }

                                        if let Some(cluster_kind) = &clusters_cfg
                                            && let pane::Content::Kline { chart, .. } =
                                                &mut state.content
                                            && let Some(c) = chart
                                        {
                                            c.set_cluster_kind(*cluster_kind);
                                        }
                                    }
                                });
                        }
                    } else if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        state.settings.visual_config = Some(cfg);
                        state.content.change_visual_config(cfg);
                    }
                }
                pane::Message::SwitchLinkGroup(pane, group) => {
                    if group.is_none() {
                        if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                            state.link_group = None;
                        }
                        return (Task::none(), None);
                    }

                    let maybe_ticker_info = self
                        .iter_all_panes(main_window.id)
                        .filter(|(w, p, _)| !(*w == window && *p == pane))
                        .find_map(|(_, _, other_state)| {
                            if other_state.link_group == group {
                                other_state.stream_pair()
                            } else {
                                None
                            }
                        });

                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        state.link_group = group;
                        state.modal = None;

                        if let Some(ticker_info) = maybe_ticker_info
                            && state.stream_pair() != Some(ticker_info)
                        {
                            let content = state.content.identifier_str();

                            match state.set_content_and_streams(ticker_info, &content) {
                                Ok(streams) => {
                                    let pane_id = state.unique_id();
                                    self.streams.extend(streams.iter());

                                    for stream in &streams {
                                        if let StreamKind::Kline { .. } = stream {
                                            return (
                                                kline_fetch_task(
                                                    *layout_id, pane_id, *stream, None, None,
                                                ),
                                                None,
                                            );
                                        }
                                    }
                                }
                                Err(err) => {
                                    state.status = pane::Status::Ready;
                                    state.notifications.push(Toast::error(err.to_string()));
                                }
                            }
                        }
                    }
                }
                pane::Message::Popout => {
                    return (self.popout_pane(main_window), None);
                }
                pane::Message::Merge => {
                    return (self.merge_pane(main_window), None);
                }
                pane::Message::ToggleIndicator(pane, indicator) => {
                    if let Some(pane_state) = self.get_mut_pane(main_window.id, window, pane) {
                        pane_state.content.toggle_indicator(indicator);
                    }
                }
                pane::Message::DeleteNotification(pane, idx) => {
                    if let Some(pane_state) = self.get_mut_pane(main_window.id, window, pane) {
                        pane_state.notifications.remove(idx);
                    }
                }
                pane::Message::ReorderIndicator(pane, event) => {
                    if let Some(pane_state) = self.get_mut_pane(main_window.id, window, pane) {
                        pane_state.content.reorder_indicators(&event);
                    }
                }
                pane::Message::ClusterKindSelected(pane, cluster_kind) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let pane::Content::Kline { chart, kind, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_cluster_kind(cluster_kind);
                        *kind = c.kind.clone();
                    }
                }
                pane::Message::ClusterScalingSelected(pane, scaling) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let pane::Content::Kline { chart, kind, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_cluster_scaling(scaling);
                        *kind = c.kind.clone();
                    }
                }
                pane::Message::CandleWidthRatioChanged(pane, ratio) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let pane::Content::Kline { chart, kind, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_candle_width_ratio(ratio);
                        *kind = c.kind.clone();
                    }
                }
                pane::Message::ClusterWidthFactorChanged(pane, factor) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let pane::Content::Kline { chart, kind, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_cluster_width_factor(factor);
                        *kind = c.kind.clone();
                    }
                }
                pane::Message::CandleBodyRatioChanged(pane, ratio) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let pane::Content::Kline { chart, kind, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_candle_body_ratio(ratio);
                        *kind = c.kind.clone();
                    }
                }
                pane::Message::WickThicknessChanged(pane, thickness) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let pane::Content::Kline { chart, kind, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_wick_thickness(thickness);
                        *kind = c.kind.clone();
                    }
                }
                pane::Message::CellWidthChanged(pane, width) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let pane::Content::Kline { chart, kind, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_cell_width(width);
                        *kind = c.kind.clone();
                    }
                }
                pane::Message::MinCellWidthChanged(pane, width) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let pane::Content::Kline { chart, kind, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_min_cell_width(width);
                        *kind = c.kind.clone();
                    }
                }
                pane::Message::MaxCellWidthChanged(pane, width) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let pane::Content::Kline { chart, kind, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_max_cell_width(width);
                        *kind = c.kind.clone();
                    }
                }
                pane::Message::CandleSpacingFactorChanged(pane, factor) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let pane::Content::Kline { chart, kind, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_candle_spacing_factor(factor);
                        *kind = c.kind.clone();
                    }
                }
                pane::Message::StudyConfigurator(pane, study_msg) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        match study_msg {
                            StudyMessage::Footprint(message) => {
                                if let pane::Content::Kline { chart, kind, .. } = &mut state.content
                                    && let Some(c) = chart
                                {
                                    c.update_study_configurator(message);
                                    *kind = c.kind.clone();
                                }
                            }

                            StudyMessage::Heatmap(message) => {
                                if let pane::Content::Heatmap { chart, studies, .. } =
                                    &mut state.content
                                    && let Some(c) = chart
                                {
                                    c.update_study_configurator(message);
                                    *studies = c.studies.clone();
                                }
                            }
                        }
                    }
                }
                pane::Message::StreamModifierChanged(pane, message) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane)
                        && let Some(pane::Modal::StreamModifier(mut modifier)) = state.modal
                    {
                        let action = modifier.update(message);

                        match action {
                            Some(modal::stream::Action::TabSelected(tab)) => {
                                modifier.tab = tab;

                                state.modal = Some(pane::Modal::StreamModifier(modifier));
                            }
                            Some(modal::stream::Action::BasisSelected(new_basis)) => {
                                modifier.update_kind_with_basis(new_basis);

                                state.modal = Some(pane::Modal::StreamModifier(modifier));
                                state.settings.selected_basis = Some(new_basis);

                                if let pane::Content::Heatmap { ref mut chart, .. } = state.content
                                {
                                    if let Some(c) = chart {
                                        c.set_basis(new_basis);
                                    }

                                    if let Some(stream_type) =
                                        state.streams.ready_iter_mut().and_then(|mut iter| {
                                            iter.find(|stream| {
                                                matches!(stream, StreamKind::DepthAndTrades { .. })
                                            })
                                        })
                                        && let StreamKind::DepthAndTrades {
                                            push_freq,
                                            ticker_info,
                                            ..
                                        } = stream_type
                                        && ticker_info.exchange().is_custom_push_freq()
                                    {
                                        match new_basis {
                                            Basis::Time(tf) => {
                                                *push_freq = PushFrequency::Custom(tf)
                                            }
                                            Basis::Tick(_) => {
                                                *push_freq = PushFrequency::ServerDefault
                                            }
                                        }

                                        return (self.refresh_streams(main_window.id), None);
                                    }

                                    return (Task::none(), None);
                                }

                                if let Some(ticker_info) = state.stream_pair() {
                                    let chart_kind = state.content.chart_kind().unwrap_or_default();
                                    let is_footprint = matches!(
                                        chart_kind,
                                        data::chart::KlineChartKind::Footprint { .. }
                                    );

                                    match new_basis {
                                        Basis::Time(new_tf) => {
                                            let kline_stream = StreamKind::Kline {
                                                ticker_info,
                                                timeframe: new_tf,
                                            };

                                            let mut streams = vec![kline_stream];

                                            if is_footprint {
                                                streams.push(StreamKind::DepthAndTrades {
                                                    ticker_info,
                                                    depth_aggr: if ticker_info
                                                        .exchange()
                                                        .is_depth_client_aggr()
                                                    {
                                                        StreamTicksize::Client
                                                    } else {
                                                        StreamTicksize::ServerSide(
                                                            state
                                                                .settings
                                                                .tick_multiply
                                                                .unwrap_or(TickMultiplier(1)),
                                                        )
                                                    },
                                                    push_freq: PushFrequency::ServerDefault,
                                                });
                                            }

                                            state.streams = ResolvedStream::Ready(streams);
                                            let pane_id = state.unique_id();

                                            if let pane::Content::Kline { .. } = &state.content {
                                                let task = kline_fetch_task(
                                                    *layout_id,
                                                    pane_id,
                                                    kline_stream,
                                                    None,
                                                    None,
                                                );
                                                return (
                                                    self.refresh_streams(main_window.id)
                                                        .chain(task),
                                                    None,
                                                );
                                            }
                                        }
                                        Basis::Tick(interval) => {
                                            let exchange = ticker_info.exchange();

                                            let depth_aggr = if exchange.is_depth_client_aggr() {
                                                StreamTicksize::Client
                                            } else {
                                                StreamTicksize::ServerSide(
                                                    state
                                                        .settings
                                                        .tick_multiply
                                                        .unwrap_or(TickMultiplier(1)),
                                                )
                                            };

                                            let streams = vec![StreamKind::DepthAndTrades {
                                                ticker_info,
                                                depth_aggr,
                                                push_freq: PushFrequency::ServerDefault,
                                            }];

                                            state.streams = ResolvedStream::Ready(streams);

                                            if let Some(pane_state) =
                                                self.get_mut_pane(main_window.id, window, pane)
                                                && let pane::Content::Kline { chart, .. } =
                                                    &mut pane_state.content
                                                && let Some(c) = chart
                                            {
                                                c.set_tick_basis(interval);
                                            }
                                        }
                                    }
                                }

                                return (self.refresh_streams(main_window.id), None);
                            }
                            Some(modal::stream::Action::TicksizeSelected(new_multiplier)) => {
                                modifier.update_kind_with_multiplier(new_multiplier);

                                state.modal = Some(pane::Modal::StreamModifier(modifier));
                                state.settings.tick_multiply = Some(new_multiplier);

                                if let Some(ticker_info) = state.stream_pair() {
                                    match state.content {
                                        pane::Content::Kline {
                                            chart: Some(ref mut c),
                                            ..
                                        } => {
                                            c.change_tick_size(
                                                new_multiplier
                                                    .multiply_with_min_tick_size(ticker_info),
                                            );

                                            c.reset_request_handler();
                                        }
                                        pane::Content::Heatmap {
                                            chart: Some(ref mut c),
                                            ..
                                        } => {
                                            c.change_tick_size(
                                                new_multiplier
                                                    .multiply_with_min_tick_size(ticker_info),
                                            );
                                        }
                                        pane::Content::Ladder(Some(ref mut panel)) => {
                                            panel.set_tick_size(
                                                new_multiplier
                                                    .multiply_with_min_tick_size(ticker_info),
                                            );
                                        }
                                        _ => {}
                                    }
                                }

                                let is_clientside_aggr = state
                                    .stream_pair()
                                    .map(|ticker_info| {
                                        ticker_info.exchange().is_depth_client_aggr()
                                    })
                                    .unwrap_or(false);

                                let desired_aggr = if is_clientside_aggr {
                                    StreamTicksize::Client
                                } else {
                                    StreamTicksize::ServerSide(new_multiplier)
                                };

                                if let Some(mut iter) = state.streams.ready_iter_mut() {
                                    for stream in &mut iter {
                                        if let StreamKind::DepthAndTrades { depth_aggr, .. } =
                                            stream
                                        {
                                            *depth_aggr = desired_aggr;
                                        }
                                    }
                                }

                                if !is_clientside_aggr {
                                    return (self.refresh_streams(main_window.id), None);
                                }
                            }
                            None => {
                                state.modal = Some(pane::Modal::StreamModifier(modifier));
                            }
                        }
                    }
                }
            },
            Message::ChangePaneStatus(pane_id, status) => {
                if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window.id, pane_id) {
                    pane_state.status = status;
                }
            }
            Message::DistributeFetchedData {
                layout_id,
                pane_id,
                data,
                stream,
            } => {
                return (
                    Task::none(),
                    Some(Event::DistributeFetchedData {
                        layout_id,
                        pane_id,
                        data,
                        stream,
                    }),
                );
            }
            Message::ResolveStreams(pane_id, streams) => {
                return (
                    Task::none(),
                    Some(Event::ResolveStreams { pane_id, streams }),
                );
            }
            Message::Notification(toast) => {
                return (Task::none(), Some(Event::Notification(toast)));
            }
        }

        (Task::none(), None)
    }

    fn new_pane(
        &mut self,
        axis: pane_grid::Axis,
        main_window: &Window,
        pane_state: Option<pane::State>,
    ) -> Task<Message> {
        if self
            .focus
            .filter(|(window, _)| *window == main_window.id)
            .is_some()
        {
            // If there is any focused pane on main window, split it
            return self.split_pane(axis, main_window);
        } else {
            // If there is no focused pane, split the last pane or create a new empty grid
            let pane = self.panes.iter().last().map(|(pane, _)| pane).copied();

            if let Some(pane) = pane {
                let result = self.panes.split(axis, pane, pane_state.unwrap_or_default());

                if let Some((pane, _)) = result {
                    return self.focus_pane(main_window.id, pane);
                }
            } else {
                let (state, pane) = pane_grid::State::new(pane_state.unwrap_or_default());
                self.panes = state;

                return self.focus_pane(main_window.id, pane);
            }
        }

        Task::none()
    }

    fn focus_pane(&mut self, window: window::Id, pane: pane_grid::Pane) -> Task<Message> {
        if self.focus != Some((window, pane)) {
            self.focus = Some((window, pane));
        }

        Task::none()
    }

    fn split_pane(&mut self, axis: pane_grid::Axis, main_window: &Window) -> Task<Message> {
        if let Some((window, pane)) = self.focus
            && window == main_window.id
        {
            let result = self.panes.split(axis, pane, pane::State::new());

            if let Some((pane, _)) = result {
                return self.focus_pane(main_window.id, pane);
            }
        }

        Task::none()
    }

    fn popout_pane(&mut self, main_window: &Window) -> Task<Message> {
        if let Some((_, id)) = self.focus.take()
            && let Some((pane, _)) = self.panes.close(id)
        {
            let (window, task) = window::open(window::Settings {
                position: main_window
                    .position
                    .map(|point| window::Position::Specific(point + Vector::new(20.0, 20.0)))
                    .unwrap_or_default(),
                exit_on_close_request: false,
                min_size: Some(iced::Size::new(400.0, 300.0)),
                ..window::settings()
            });

            let (state, id) = pane_grid::State::new(pane);
            self.popout.insert(window, (state, WindowSpec::default()));

            return task.then(move |window| {
                Task::done(Message::Pane(window, pane::Message::PaneClicked(id)))
            });
        }

        Task::none()
    }

    fn merge_pane(&mut self, main_window: &Window) -> Task<Message> {
        if let Some((window, pane)) = self.focus.take()
            && let Some(pane_state) = self
                .popout
                .remove(&window)
                .and_then(|(mut panes, _)| panes.panes.remove(&pane))
        {
            let task = self.new_pane(pane_grid::Axis::Horizontal, main_window, Some(pane_state));

            return Task::batch(vec![window::close(window), task]);
        }

        Task::none()
    }

    pub fn get_pane(
        &self,
        main_window: window::Id,
        window: window::Id,
        pane: pane_grid::Pane,
    ) -> Option<&pane::State> {
        if main_window == window {
            self.panes.get(pane)
        } else {
            self.popout
                .get(&window)
                .and_then(|(panes, _)| panes.get(pane))
        }
    }

    fn get_mut_pane(
        &mut self,
        main_window: window::Id,
        window: window::Id,
        pane: pane_grid::Pane,
    ) -> Option<&mut pane::State> {
        if main_window == window {
            self.panes.get_mut(pane)
        } else {
            self.popout
                .get_mut(&window)
                .and_then(|(panes, _)| panes.get_mut(pane))
        }
    }

    fn get_mut_pane_state_by_uuid(
        &mut self,
        main_window: window::Id,
        uuid: uuid::Uuid,
    ) -> Option<&mut pane::State> {
        self.iter_all_panes_mut(main_window)
            .find(|(_, _, state)| state.unique_id() == uuid)
            .map(|(_, _, state)| state)
    }

    fn iter_all_panes(
        &self,
        main_window: window::Id,
    ) -> impl Iterator<Item = (window::Id, pane_grid::Pane, &pane::State)> {
        self.panes
            .iter()
            .map(move |(pane, state)| (main_window, *pane, state))
            .chain(self.popout.iter().flat_map(|(window_id, (panes, _))| {
                panes.iter().map(|(pane, state)| (*window_id, *pane, state))
            }))
    }

    fn iter_all_panes_mut(
        &mut self,
        main_window: window::Id,
    ) -> impl Iterator<Item = (window::Id, pane_grid::Pane, &mut pane::State)> {
        self.panes
            .iter_mut()
            .map(move |(pane, state)| (main_window, *pane, state))
            .chain(self.popout.iter_mut().flat_map(|(window_id, (panes, _))| {
                panes
                    .iter_mut()
                    .map(|(pane, state)| (*window_id, *pane, state))
            }))
    }

    pub fn view<'a>(
        &'a self,
        main_window: &'a Window,
        timezone: UserTimezone,
    ) -> Element<'a, Message> {
        let pane_grid: Element<_> = PaneGrid::new(&self.panes, |id, pane, maximized| {
            let is_focused = self.focus == Some((main_window.id, id));
            pane.view(
                id,
                self.panes.len(),
                is_focused,
                maximized,
                main_window.id,
                main_window,
                timezone,
            )
        })
        .min_size(240)
        .on_click(pane::Message::PaneClicked)
        .on_drag(pane::Message::PaneDragged)
        .on_resize(8, pane::Message::PaneResized)
        .spacing(6)
        .style(style::pane_grid)
        .into();

        pane_grid.map(move |message| Message::Pane(main_window.id, message))
    }

    pub fn view_window<'a>(
        &'a self,
        window: window::Id,
        main_window: &'a Window,
        timezone: UserTimezone,
    ) -> Element<'a, Message> {
        if let Some((state, _)) = self.popout.get(&window) {
            let content = container(
                PaneGrid::new(state, |id, pane, _maximized| {
                    let is_focused = self.focus == Some((window, id));
                    pane.view(
                        id,
                        state.len(),
                        is_focused,
                        false,
                        window,
                        main_window,
                        timezone,
                    )
                })
                .on_click(pane::Message::PaneClicked),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8);

            Element::new(content).map(move |message| Message::Pane(window, message))
        } else {
            Element::new(center("No pane found for window"))
                .map(move |message| Message::Pane(window, message))
        }
    }

    pub fn go_back(&mut self, main_window: window::Id) -> bool {
        let Some((window, pane)) = self.focus else {
            return false;
        };

        let Some(state) = self.get_mut_pane(main_window, window, pane) else {
            return false;
        };

        if state.modal.is_some() {
            state.modal = None;
            return true;
        }
        false
    }

    fn handle_error(
        &mut self,
        pane_id: Option<uuid::Uuid>,
        err: &DashboardError,
        main_window: window::Id,
    ) -> Task<Message> {
        match pane_id {
            Some(id) => {
                if let Some(state) = self.get_mut_pane_state_by_uuid(main_window, id) {
                    state.status = pane::Status::Ready;
                    state.notifications.push(Toast::error(err.to_string()));
                }
                Task::none()
            }
            _ => Task::done(Message::Notification(Toast::error(err.to_string()))),
        }
    }

    fn init_pane(
        &mut self,
        main_window: window::Id,
        window: window::Id,
        selected_pane: pane_grid::Pane,
        ticker_info: TickerInfo,
        content: &str,
    ) -> Task<Message> {
        if let Some(state) = self.get_mut_pane(main_window, window, selected_pane) {
            match state.set_content_and_streams(ticker_info, content) {
                Ok(streams) => {
                    let pane_id = state.unique_id();
                    self.streams.extend(streams.iter());

                    for stream in &streams {
                        if let StreamKind::Kline { .. } = stream {
                            return kline_fetch_task(self.layout_id, pane_id, *stream, None, None);
                        }
                    }
                }
                Err(err) => {
                    state.status = pane::Status::Ready;
                    state.notifications.push(Toast::error(err.to_string()));
                }
            }
        }

        Task::none()
    }

    pub fn init_focused_pane(
        &mut self,
        main_window: window::Id,
        ticker_info: TickerInfo,
        content: &str,
    ) -> Task<Message> {
        if self.focus.is_none()
            && self.panes.len() == 1
            && let Some((pane_id, _)) = self.panes.iter().next()
        {
            self.focus = Some((main_window, *pane_id));
        }

        if let Some((window, selected_pane)) = self.focus
            && let Some(state) = self.get_mut_pane(main_window, window, selected_pane)
        {
            let previous_ticker = state.stream_pair();
            if previous_ticker.is_some() && previous_ticker != Some(ticker_info) {
                state.link_group = None;
            }

            match state.set_content_and_streams(ticker_info, content) {
                Ok(streams) => {
                    let pane_id = state.unique_id();
                    self.streams.extend(streams.iter());

                    for stream in &streams {
                        if let StreamKind::Kline { .. } = stream {
                            return kline_fetch_task(self.layout_id, pane_id, *stream, None, None);
                        }
                    }
                }
                Err(err) => {
                    state.status = pane::Status::Ready;
                    state.notifications.push(Toast::error(err.to_string()));
                }
            }
            return Task::none();
        }

        Task::done(Message::Notification(Toast::warn(
            "No focused pane found".to_string(),
        )))
    }

    pub fn switch_tickers_in_group(
        &mut self,
        main_window: window::Id,
        ticker_info: TickerInfo,
    ) -> Task<Message> {
        if self.focus.is_none()
            && self.panes.len() == 1
            && let Some((pane_id, _)) = self.panes.iter().next()
        {
            self.focus = Some((main_window, *pane_id));
        }

        let link_group = self.focus.and_then(|(window, pane)| {
            self.get_pane(main_window, window, pane)
                .and_then(|state| state.link_group)
        });

        if let Some(group) = link_group {
            let pane_infos: Vec<(window::Id, pane_grid::Pane, String)> = self
                .iter_all_panes_mut(main_window)
                .filter_map(|(window, pane, state)| {
                    if state.link_group == Some(group) {
                        Some((window, pane, state.content.identifier_str()))
                    } else {
                        None
                    }
                })
                .collect();

            let tasks: Vec<Task<Message>> = pane_infos
                .iter()
                .map(|(window, pane, content)| {
                    self.init_pane(main_window, *window, *pane, ticker_info, content)
                })
                .collect();

            Task::batch(tasks)
        } else if let Some((window, pane)) = self.focus {
            if let Some(state) = self.get_mut_pane(main_window, window, pane) {
                let content_kind = &state.content.identifier_str();
                self.init_focused_pane(main_window, ticker_info, content_kind)
            } else {
                Task::done(Message::Notification(Toast::warn(
                    "Couldn't get focused pane's content".to_string(),
                )))
            }
        } else {
            Task::done(Message::Notification(Toast::warn(
                "No link group or focused pane found".to_string(),
            )))
        }
    }

    pub fn toggle_trade_fetch(&mut self, is_enabled: bool, main_window: &Window) {
        exchange::fetcher::toggle_trade_fetch(is_enabled);

        self.iter_all_panes_mut(main_window.id)
            .for_each(|(_, _, state)| {
                if let pane::Content::Kline { chart, kind, .. } = &mut state.content
                    && matches!(kind, data::chart::KlineChartKind::Footprint { .. })
                    && let Some(c) = chart
                {
                    c.reset_request_handler();

                    if !is_enabled {
                        state.status = pane::Status::Ready;
                    }
                }
            });
    }

    pub fn distribute_fetched_data(
        &mut self,
        main_window: window::Id,
        pane_id: uuid::Uuid,
        data: FetchedData,
        stream_type: StreamKind,
    ) -> Task<Message> {
        // Persist data to database before distributing to in-memory structures (dual-write)
        let ticker_info = match &stream_type {
            StreamKind::Kline { ticker_info, .. } => Some(ticker_info),
            StreamKind::DepthAndTrades { ticker_info, .. } => Some(ticker_info),
        };

        if let Some(ticker_info) = ticker_info {
            self.persist_fetched_data(ticker_info, &data, &stream_type);
        }

        match data {
            FetchedData::Trades { batch, until_time } => {
                let last_trade_time = batch.last().map_or(0, |trade| trade.time);

                if last_trade_time < until_time {
                    if let Err(reason) =
                        self.insert_fetched_trades(main_window, pane_id, &batch, false)
                    {
                        return self.handle_error(Some(pane_id), &reason, main_window);
                    }
                } else {
                    let filtered_batch = batch
                        .iter()
                        .filter(|trade| trade.time <= until_time)
                        .copied()
                        .collect::<Vec<_>>();

                    if let Err(reason) =
                        self.insert_fetched_trades(main_window, pane_id, &filtered_batch, true)
                    {
                        return self.handle_error(Some(pane_id), &reason, main_window);
                    }
                }
            }
            FetchedData::Klines { data, req_id } => {
                if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window, pane_id) {
                    pane_state.status = pane::Status::Ready;

                    if let StreamKind::Kline { timeframe, .. } = stream_type {
                        pane_state.insert_klines_vec(req_id, timeframe, &data);
                    }
                }
            }
            FetchedData::OI { data, req_id } => {
                if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window, pane_id) {
                    pane_state.status = pane::Status::Ready;

                    if let StreamKind::Kline { .. } = stream_type {
                        pane_state.insert_oi_vec(req_id, &data);
                    }
                }
            }
        }

        Task::none()
    }

    /// Set database manager for dual-write persistence
    pub fn set_db_manager(&mut self, db_manager: Option<std::sync::Arc<data::db::DatabaseManager>>) {
        self.db_manager = db_manager;
    }

    /// Persist fetched data to database before distributing to panes
    fn persist_fetched_data(
        &self,
        ticker_info: &TickerInfo,
        data: &FetchedData,
        stream_type: &StreamKind,
    ) {
        if let Some(ref db_manager) = self.db_manager {
            let result = match data {
                FetchedData::Trades { batch, .. } => {
                    use data::db::TradesCRUD;
                    db_manager.insert_trades(ticker_info, batch)
                        .map(|count| {
                            if count > 0 {
                                log::info!("✓ Persisted {} trades to database for {:?}", count, ticker_info.ticker);
                            }
                        })
                }
                FetchedData::Klines { data: klines, .. } => {
                    if let StreamKind::Kline { timeframe, .. } = stream_type {
                        use data::db::KlinesCRUD;
                        db_manager.insert_klines(ticker_info, *timeframe, klines)
                            .map(|count| {
                                if count > 0 {
                                    log::info!("✓ Persisted {} klines to database for {:?} {:?}", count, ticker_info.ticker, timeframe);
                                }
                            })
                    } else {
                        Ok(())
                    }
                }
                FetchedData::OI { .. } => {
                    // Open interest persistence can be added in the future
                    Ok(())
                }
            };

            if let Err(e) = result {
                log::error!("Failed to persist data to database: {}", e);
            }
        }
    }

    fn insert_fetched_trades(
        &mut self,
        main_window: window::Id,
        pane_id: uuid::Uuid,
        trades: &[Trade],
        is_batches_done: bool,
    ) -> Result<(), DashboardError> {
        let pane_state = self
            .get_mut_pane_state_by_uuid(main_window, pane_id)
            .ok_or_else(|| {
                DashboardError::Unknown(
                    "No matching pane state found for fetched trades".to_string(),
                )
            })?;

        match &mut pane_state.status {
            pane::Status::Loading(pane::InfoType::FetchingTrades(count)) => {
                *count += trades.len();
            }
            _ => {
                pane_state.status =
                    pane::Status::Loading(pane::InfoType::FetchingTrades(trades.len()));
            }
        }

        match &mut pane_state.content {
            pane::Content::Kline { chart, .. } => {
                if let Some(c) = chart {
                    c.insert_raw_trades(trades.to_owned(), is_batches_done);

                    if is_batches_done {
                        pane_state.status = pane::Status::Ready;
                    }
                    Ok(())
                } else {
                    Err(DashboardError::Unknown(
                        "fetched trades but no chart found".to_string(),
                    ))
                }
            }
            _ => Err(DashboardError::Unknown(
                "No matching chart found for fetched trades".to_string(),
            )),
        }
    }

    pub fn update_latest_klines(
        &mut self,
        stream: &StreamKind,
        kline: &Kline,
        main_window: window::Id,
    ) -> Task<Message> {
        let mut found_match = false;

        self.iter_all_panes_mut(main_window)
            .for_each(|(_, _, pane_state)| {
                if pane_state.matches_stream(stream) {
                    if let pane::Content::Kline { chart, .. } = &mut pane_state.content
                        && let Some(c) = chart
                    {
                        c.update_latest_kline(kline);
                    }

                    found_match = true;
                }
            });

        if found_match {
            Task::none()
        } else {
            log::debug!("{stream:?} stream had no matching panes - dropping");
            self.refresh_streams(main_window)
        }
    }

    pub fn update_depth_and_trades(
        &mut self,
        stream: &StreamKind,
        depth_update_t: u64,
        depth: &Depth,
        trades_buffer: &[Trade],
        main_window: window::Id,
    ) -> Task<Message> {
        let mut found_match = false;

        self.iter_all_panes_mut(main_window)
            .for_each(|(_, _, pane_state)| {
                if pane_state.matches_stream(stream) {
                    match &mut pane_state.content {
                        pane::Content::Heatmap { chart, .. } => {
                            if let Some(c) = chart {
                                c.insert_datapoint(trades_buffer, depth_update_t, depth);
                            }
                        }
                        pane::Content::Kline { chart, .. } => {
                            if let Some(c) = chart {
                                c.insert_trades_buffer(trades_buffer);
                            }
                        }
                        pane::Content::TimeAndSales(panel) => {
                            if let Some(p) = panel {
                                p.insert_buffer(trades_buffer);
                            }
                        }
                        pane::Content::Ladder(panel) => {
                            if let Some(panel) = panel {
                                panel.insert_buffers(depth_update_t, depth, trades_buffer);
                            }
                        }
                        _ => {
                            log::error!("No chart found for the stream: {stream:?}");
                        }
                    }
                    found_match = true;
                }
            });

        if found_match {
            Task::none()
        } else {
            log::debug!("No matching pane found for the stream: {stream:?}");
            self.refresh_streams(main_window)
        }
    }

    pub fn invalidate_all_panes(&mut self, main_window: window::Id) {
        self.iter_all_panes_mut(main_window)
            .for_each(|(_, _, state)| {
                let _ = state.invalidate(Instant::now());
            });
    }

    pub fn tick(&mut self, now: Instant, main_window: window::Id) -> Task<Message> {
        let mut tasks = vec![];
        let layout_id = self.layout_id;

        self.iter_all_panes_mut(main_window)
            .for_each(|(_window_id, _pane, state)| match state.tick(now) {
                Some(pane::Action::Chart(action)) => match action {
                    chart::Action::ErrorOccurred(err) => {
                        state.status = pane::Status::Ready;
                        state.notifications.push(Toast::error(err.to_string()));
                    }
                    chart::Action::FetchRequested(req_id, fetch) => {
                        tasks.push(request_fetch(state, layout_id, req_id, fetch));
                    }
                },
                Some(pane::Action::Panel(_action)) => {}
                Some(pane::Action::ResolveStreams(streams)) => {
                    tasks.push(Task::done(Message::ResolveStreams(
                        state.unique_id(),
                        streams,
                    )));
                }
                Some(pane::Action::ResolveContent) => {
                    if let Some(ticker_info) = state.stream_pair() {
                        state
                            .set_content_and_streams(ticker_info, &state.content.identifier_str())
                            .ok();
                    }
                }
                None => {}
            });

        Task::batch(tasks)
    }

    pub fn resolve_streams(
        &mut self,
        main_window: window::Id,
        pane_id: uuid::Uuid,
        streams: Vec<StreamKind>,
    ) -> Task<Message> {
        if let Some(state) = self.get_mut_pane_state_by_uuid(main_window, pane_id) {
            state.streams = ResolvedStream::Ready(streams.clone());
        }
        self.refresh_streams(main_window)
    }

    pub fn market_subscriptions(&self) -> Subscription<exchange::Event> {
        let unique_streams = self
            .streams
            .combined_used()
            .flat_map(|(exchange, specs)| {
                let mut subs = vec![];

                if !specs.depth.is_empty() {
                    let depth_subs = specs
                        .depth
                        .iter()
                        .map(|(ticker, aggr, push_freq)| {
                            let tick_mltp = match aggr {
                                StreamTicksize::Client => None,
                                StreamTicksize::ServerSide(tick_mltp) => Some(*tick_mltp),
                            };
                            depth_subscription(*ticker, tick_mltp, *push_freq)
                        })
                        .collect::<Vec<_>>();

                    if !depth_subs.is_empty() {
                        subs.push(Subscription::batch(depth_subs));
                    }
                }

                let kline_params = specs
                    .kline
                    .iter()
                    .map(|(ticker, timeframe)| (*ticker, *timeframe))
                    .collect::<Vec<_>>();

                if !kline_params.is_empty() {
                    subs.push(kline_subscription(exchange, kline_params));
                }

                subs
            })
            .collect::<Vec<Subscription<exchange::Event>>>();

        Subscription::batch(unique_streams)
    }

    fn refresh_streams(&mut self, main_window: window::Id) -> Task<Message> {
        let all_pane_streams = self
            .iter_all_panes(main_window)
            .flat_map(|(_, _, pane_state)| pane_state.streams.ready_iter().into_iter().flatten());
        self.streams = UniqueStreams::from(all_pane_streams);

        Task::none()
    }
}

fn request_fetch(
    state: &mut pane::State,
    layout_id: uuid::Uuid,
    req_id: uuid::Uuid,
    fetch: FetchRange,
) -> Task<Message> {
    let pane_id = state.unique_id();

    match fetch {
        FetchRange::Kline(from, to) => {
            let kline_stream = {
                state.streams.find_ready_map(|stream| {
                    if let StreamKind::Kline { .. } = stream {
                        Some((*stream, pane_id))
                    } else {
                        None
                    }
                })
            };

            if let Some((stream, pane_uid)) = kline_stream {
                return kline_fetch_task(
                    layout_id,
                    pane_uid,
                    stream,
                    Some(req_id),
                    Some((from, to)),
                );
            }
        }
        FetchRange::OpenInterest(from, to) => {
            let kline_stream = {
                state.streams.find_ready_map(|stream| {
                    if let StreamKind::Kline { .. } = stream {
                        Some((*stream, pane_id))
                    } else {
                        None
                    }
                })
            };

            if let Some((stream, pane_uid)) = kline_stream {
                return oi_fetch_task(layout_id, pane_uid, stream, Some(req_id), Some((from, to)));
            }
        }
        FetchRange::Trades(from_time, to_time) => {
            let trade_info = state.streams.find_ready_map(|stream| {
                if let StreamKind::DepthAndTrades { ticker_info, .. } = stream {
                    Some((*ticker_info, pane_id, *stream))
                } else {
                    None
                }
            });

            if let Some((ticker_info, pane_id, stream)) = trade_info {
                let is_binance = matches!(
                    ticker_info.exchange(),
                    Exchange::BinanceSpot | Exchange::BinanceLinear | Exchange::BinanceInverse
                );

                if is_binance {
                    let data_path = data::data_path(Some("market_data/binance/"));

                    let (task, handle) = Task::sip(
                        fetch_trades_batched(ticker_info, from_time, to_time, data_path),
                        move |batch| {
                            let data = FetchedData::Trades {
                                batch,
                                until_time: to_time,
                            };
                            Message::DistributeFetchedData {
                                layout_id,
                                pane_id,
                                data,
                                stream,
                            }
                        },
                        move |result| match result {
                            Ok(()) => Message::ChangePaneStatus(pane_id, pane::Status::Ready),
                            Err(err) => Message::ErrorOccurred(
                                Some(pane_id),
                                DashboardError::Fetch(err.to_string()),
                            ),
                        },
                    )
                    .abortable();

                    if let pane::Content::Kline { chart, .. } = &mut state.content
                        && let Some(c) = chart
                    {
                        c.set_handle(handle.abort_on_drop());
                    }

                    return task;
                }
            }
        }
    }

    Task::none()
}

fn oi_fetch_task(
    layout_id: uuid::Uuid,
    pane_id: uuid::Uuid,
    stream: StreamKind,
    req_id: Option<uuid::Uuid>,
    range: Option<(u64, u64)>,
) -> Task<Message> {
    let update_status = Task::done(Message::ChangePaneStatus(
        pane_id,
        pane::Status::Loading(pane::InfoType::FetchingOI),
    ));

    let fetch_task = match stream {
        StreamKind::Kline {
            ticker_info,
            timeframe,
        } => Task::perform(
            adapter::fetch_open_interest(ticker_info.ticker, timeframe, range)
                .map_err(|err| format!("{err}")),
            move |result| match result {
                Ok(oi) => {
                    let data = FetchedData::OI { data: oi, req_id };
                    Message::DistributeFetchedData {
                        layout_id,
                        pane_id,
                        data,
                        stream,
                    }
                }
                Err(err) => Message::ErrorOccurred(Some(pane_id), DashboardError::Fetch(err)),
            },
        ),
        _ => Task::none(),
    };

    update_status.chain(fetch_task)
}

fn kline_fetch_task(
    layout_id: uuid::Uuid,
    pane_id: uuid::Uuid,
    stream: StreamKind,
    req_id: Option<uuid::Uuid>,
    range: Option<(u64, u64)>,
) -> Task<Message> {
    let update_status = Task::done(Message::ChangePaneStatus(
        pane_id,
        pane::Status::Loading(pane::InfoType::FetchingKlines),
    ));

    let fetch_task = match stream {
        StreamKind::Kline {
            ticker_info,
            timeframe,
        } => Task::perform(
            adapter::fetch_klines(ticker_info, timeframe, range).map_err(|err| format!("{err}")),
            move |result| match result {
                Ok(klines) => {
                    let data = FetchedData::Klines {
                        data: klines,
                        req_id,
                    };
                    Message::DistributeFetchedData {
                        layout_id,
                        pane_id,
                        data,
                        stream,
                    }
                }
                Err(err) => Message::ErrorOccurred(Some(pane_id), DashboardError::Fetch(err)),
            },
        ),
        _ => Task::none(),
    };

    update_status.chain(fetch_task)
}

pub fn fetch_trades_batched(
    ticker_info: TickerInfo,
    from_time: u64,
    to_time: u64,
    data_path: PathBuf,
) -> impl Straw<(), Vec<Trade>, AdapterError> {
    sipper(async move |mut progress| {
        let mut latest_trade_t = from_time;

        while latest_trade_t < to_time {
            match binance::fetch_trades(ticker_info, latest_trade_t, data_path.clone()).await {
                Ok(batch) => {
                    if batch.is_empty() {
                        break;
                    }

                    latest_trade_t = batch.last().map_or(latest_trade_t, |trade| trade.time);

                    let () = progress.send(batch).await;
                }
                Err(err) => return Err(err),
            }
        }

        Ok(())
    })
}

pub fn depth_subscription(
    ticker_info: TickerInfo,
    tick_mlpt: Option<TickMultiplier>,
    push_freq: PushFrequency,
) -> Subscription<exchange::Event> {
    let exchange = ticker_info.exchange();

    let config = StreamConfig::new(ticker_info, exchange, tick_mlpt, push_freq);

    match exchange {
        Exchange::AsterLinear => {
            let builder = |cfg: &StreamConfig<TickerInfo>| {
                aster::connect_market_stream(cfg.id, cfg.push_freq)
            };
            Subscription::run_with(config, builder)
        }
        Exchange::BinanceSpot | Exchange::BinanceInverse | Exchange::BinanceLinear => {
            let builder = |cfg: &StreamConfig<TickerInfo>| {
                binance::connect_market_stream(cfg.id, cfg.push_freq)
            };
            Subscription::run_with(config, builder)
        }
        Exchange::BybitSpot | Exchange::BybitLinear | Exchange::BybitInverse => {
            let builder = |cfg: &StreamConfig<TickerInfo>| {
                bybit::connect_market_stream(cfg.id, cfg.push_freq)
            };
            Subscription::run_with(config, builder)
        }
        Exchange::HyperliquidSpot | Exchange::HyperliquidLinear => {
            let builder = |cfg: &StreamConfig<TickerInfo>| {
                hyperliquid::connect_market_stream(cfg.id, cfg.tick_mltp, cfg.push_freq)
            };
            Subscription::run_with(config, builder)
        }
        Exchange::OkexLinear | Exchange::OkexInverse | Exchange::OkexSpot => {
            let builder =
                |cfg: &StreamConfig<TickerInfo>| okex::connect_market_stream(cfg.id, cfg.push_freq);
            Subscription::run_with(config, builder)
        }
    }
}

pub fn kline_subscription(
    exchange: Exchange,
    kline_subs: Vec<(TickerInfo, Timeframe)>,
) -> Subscription<exchange::Event> {
    let config = StreamConfig::new(kline_subs, exchange, None, PushFrequency::ServerDefault);
    match exchange {
        Exchange::AsterLinear => {
            let builder = |cfg: &StreamConfig<Vec<(TickerInfo, Timeframe)>>| {
                aster::connect_kline_stream(cfg.id.clone(), cfg.market_type)
            };
            Subscription::run_with(config, builder)
        }
        Exchange::BinanceSpot | Exchange::BinanceInverse | Exchange::BinanceLinear => {
            let builder = |cfg: &StreamConfig<Vec<(TickerInfo, Timeframe)>>| {
                binance::connect_kline_stream(cfg.id.clone(), cfg.market_type)
            };
            Subscription::run_with(config, builder)
        }
        Exchange::BybitSpot | Exchange::BybitInverse | Exchange::BybitLinear => {
            let builder = |cfg: &StreamConfig<Vec<(TickerInfo, Timeframe)>>| {
                bybit::connect_kline_stream(cfg.id.clone(), cfg.market_type)
            };
            Subscription::run_with(config, builder)
        }
        Exchange::HyperliquidSpot | Exchange::HyperliquidLinear => {
            let builder = |cfg: &StreamConfig<Vec<(TickerInfo, Timeframe)>>| {
                hyperliquid::connect_kline_stream(cfg.id.clone(), cfg.market_type)
            };
            Subscription::run_with(config, builder)
        }
        Exchange::OkexLinear | Exchange::OkexInverse | Exchange::OkexSpot => {
            let builder = |cfg: &StreamConfig<Vec<(TickerInfo, Timeframe)>>| {
                okex::connect_kline_stream(cfg.id.clone(), cfg.market_type)
            };
            Subscription::run_with(config, builder)
        }
    }
}
