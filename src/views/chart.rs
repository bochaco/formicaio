use crate::{
    app::{ClientGlobalState, METRICS_MAX_SIZE_PER_NODE},
    server_api::{get_settings, node_metrics},
    types::{METRIC_KEY_CPU_USAGE, METRIC_KEY_MEM_USED_MB, NodeId},
};

use super::icons::IconCancel;

use charming::{
    Chart,
    component::Axis,
    datatype::{CompositeValue, DataPoint},
    element::{
        AxisLabel, AxisLine, AxisType, Formatter, ItemStyle, JsFunction, SplitLine, TextStyle,
        Tooltip, Trigger,
    },
    series::Line,
};
use gloo_timers::future::TimeoutFuture;
use leptos::{logging, prelude::*};

pub type ChartSeriesData = (Vec<(i64, f64)>, Vec<(i64, f64)>);

const CHART_MEM_SERIES_NAME: &str = "Memory (MB)";
const CHART_CPU_SERIES_NAME: &str = "CPU (%)";
const CHART_MEM_COLOR: &str = "#F98080";
const CHART_CPU_COLOR: &str = "#3F83F8";

fn build_metrics_chart(mem: &[(i64, f64)], cpu: &[(i64, f64)]) -> Chart {
    let to_dataframe = |pts: &[(i64, f64)]| -> Vec<DataPoint> {
        pts.iter()
            .map(|(ts, v)| {
                DataPoint::from(CompositeValue::from(vec![
                    CompositeValue::from(*ts),
                    CompositeValue::from(*v),
                ]))
            })
            .collect()
    };

    Chart::new()
        .tooltip(
            Tooltip::new().trigger(Trigger::Axis).formatter(Formatter::Function(
                JsFunction::new_with_args(
                    "params",
                    "var d = new Date(params[0].axisValue);
                     var pad = function(n) { return n < 10 ? '0' + n : '' + n; };
                     var date = d.getFullYear() + '-' + pad(d.getMonth()+1) + '-' + pad(d.getDate());
                     var time = pad(d.getHours()) + ':' + pad(d.getMinutes()) + ':' + pad(d.getSeconds());
                     var out = '<b>' + date + ' ' + time + '</b><br/>';
                     params.forEach(function(p) {
                         out += p.marker + ' ' + p.seriesName + ': <b>' + p.value[1].toFixed(4) + '</b><br/>';
                     });
                     return out;",
                ),
            )),
        )
        .x_axis(
            Axis::new()
                .type_(AxisType::Time)
                .axis_line(AxisLine::new().show(false))
                .split_line(SplitLine::new().show(false))
                .axis_label(
                    AxisLabel::new()
                        .color("#9CA3AF")
                        .formatter(Formatter::String("{HH}:{mm}:{ss}".to_string())),
                ),
        )
        .y_axis(
            Axis::new()
                .type_(AxisType::Value)
                .name(CHART_MEM_SERIES_NAME)
                .name_text_style(TextStyle::new().color(CHART_MEM_COLOR))
                .axis_label(AxisLabel::new().color(CHART_MEM_COLOR))
                .axis_line(AxisLine::new().show(false))
                .split_line(SplitLine::new().show(false)),
        )
        .y_axis(
            Axis::new()
                .type_(AxisType::Value)
                .name(CHART_CPU_SERIES_NAME)
                .name_text_style(TextStyle::new().color(CHART_CPU_COLOR))
                .axis_label(AxisLabel::new().color(CHART_CPU_COLOR))
                .axis_line(AxisLine::new().show(false))
                .split_line(SplitLine::new().show(false))
                .position("right"),
        )
        .series(
            Line::new()
                .name(CHART_MEM_SERIES_NAME)
                .data(to_dataframe(mem))
                .y_axis_index(0)
                .smooth(true)
                .show_symbol(false)
                .item_style(ItemStyle::new().color(CHART_MEM_COLOR)),
        )
        .series(
            Line::new()
                .name(CHART_CPU_SERIES_NAME)
                .data(to_dataframe(cpu))
                .y_axis_index(1)
                .smooth(true)
                .show_symbol(false)
                .item_style(ItemStyle::new().color(CHART_CPU_COLOR)),
        )
}

#[component]
pub fn MetricsViewerModal(
    set_render_chart: RwSignal<bool>,
    chart_data: ReadSignal<ChartSeriesData>,
) -> impl IntoView {
    let context = expect_context::<ClientGlobalState>();

    let is_active = move || {
        context
            .metrics_update_on_for
            .read()
            .map(|info| info.read().status.is_active())
            .unwrap_or(false)
    };
    let status_summary = move || {
        context
            .metrics_update_on_for
            .read()
            .map(|info| info.read().status_summary())
            .unwrap_or_default()
    };

    view! {
        <div class="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm animate-in fade-in duration-300">
            <div class="bg-slate-900 border border-slate-800 w-full rounded-2xl overflow-hidden shadow-2xl flex flex-col animate-in zoom-in-95 duration-300">
                <header class="p-4 border-b border-slate-800 flex items-center justify-between bg-slate-800/30 shrink-0">
                    <div class="flex items-center gap-3">
                        <h3 class="text-lg font-bold">
                            "Real-time Mem & CPU Metrics: "
                            <span class="text-indigo-400 font-mono">
                                {move || {
                                    context
                                        .metrics_update_on_for
                                        .read()
                                        .as_ref()
                                        .map(|n| n.read().short_node_id())
                                        .unwrap_or_default()
                                }}
                            </span>
                        </h3>
                    </div>
                    <button
                        on:click=move |_| {
                            set_render_chart.set(false);
                            context.metrics_update_on_for.set(None);
                        }
                        class="p-2 text-slate-500 hover:text-white transition-colors rounded-lg"
                    >
                        <IconCancel />
                    </button>
                </header>

                <main>
                    <NodeChartView
                        is_render_chart=Signal::derive(move || set_render_chart.get())
                        chart_data
                    />
                </main>

                <footer class="p-3 border-t border-slate-800 bg-slate-800/30 text-xs text-slate-500 flex items-center gap-2">
                    <div class=move || {
                        format!(
                            "w-2 h-2 rounded-full {}",
                            if is_active() { "bg-emerald-500 animate-pulse" } else { "bg-rose-500" },
                        )
                    } />
                    <span>
                        Node Status:
                        <span class="font-bold capitalize">{move || status_summary()}</span>
                    </span>
                </footer>

            </div>
        </div>
    }
}

#[component]
pub fn NodeChartView(
    is_render_chart: Signal<bool>,
    chart_data: ReadSignal<ChartSeriesData>,
) -> impl IntoView {
    let chart_id = "metrics_chart";

    use charming::WasmRenderer;

    let echarts = RwSignal::new_local(None::<charming::renderer::wasm_renderer::Echarts>);

    Effect::new(move |_| {
        if !*is_render_chart.read() {
            return;
        }
        match WasmRenderer::new_opt(None, None).render(chart_id, &build_metrics_chart(&[], &[])) {
            Ok(e) => echarts.update(|h| *h = Some(e)),
            Err(err) => logging::error!("[ERROR] Failed to render chart: {err}"),
        }
    });

    Effect::new(move |_| {
        if !*is_render_chart.read() {
            return;
        }
        echarts.with(|h| {
            if let Some(e) = h {
                let (mem, cpu) = chart_data.get();
                WasmRenderer::update(e, &build_metrics_chart(&mem, &cpu));
            }
        });
    });

    let no_data = move || {
        let (mem, cpu) = chart_data.get();
        mem.is_empty() && cpu.is_empty()
    };

    view! {
        <div class="relative" style="width: 100%; height: 380px;">
            <div id=chart_id style="width: 100%; height: 380px;" />
            <Show when=no_data>
                <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
                    <p class="text-slate-400 text-sm">"No metrics data available yet"</p>
                </div>
            </Show>
        </div>
    }
}

// Fetch metrics data for a given node to render the charts
pub async fn node_metrics_update(
    node_id: NodeId,
    set_chart_data: WriteSignal<ChartSeriesData>,
) -> Result<(), ServerFnError> {
    logging::log!("Retriving node metrics from node {node_id}...");

    let polling_freq_millis =
        get_settings().await?.nodes_metrics_polling_freq.as_secs() as u32 * 2000;

    // use context to check if we should stop retrieving the metrics
    let context = expect_context::<ClientGlobalState>();
    let mut since = None;
    set_chart_data.update(|data| *data = (vec![], vec![]));

    while let Some(true) = context
        .metrics_update_on_for
        .get_untracked()
        .map(|node_info| node_info.read_untracked().node_id == node_id)
    {
        let update = node_metrics(node_id.clone(), since).await?;

        match (
            update.get(METRIC_KEY_MEM_USED_MB),
            update.get(METRIC_KEY_CPU_USAGE),
        ) {
            (Some(mem), Some(cpu)) if !mem.is_empty() && !cpu.is_empty() => {
                since = mem.last().map(|m| m.timestamp);
                set_chart_data.update(|(m, c)| {
                    m.extend(
                        mem.iter()
                            .map(|v| (v.timestamp, v.value.parse::<f64>().unwrap_or_default())),
                    );
                    c.extend(
                        cpu.iter()
                            .map(|v| (v.timestamp, v.value.parse::<f64>().unwrap_or_default())),
                    );

                    // remove items if they exceed the max size
                    if let Some(delta) = m.len().checked_sub(METRICS_MAX_SIZE_PER_NODE) {
                        m.drain(0..delta);
                    }
                    if let Some(delta) = c.len().checked_sub(METRICS_MAX_SIZE_PER_NODE) {
                        c.drain(0..delta);
                    }
                });
            }
            _ => (),
        }

        // FIXME: shortcircuit the delay if the flag is set to off
        TimeoutFuture::new(polling_freq_millis).await;
    }

    logging::log!("Stopped node metrics update from node {node_id}.");
    Ok(())
}
