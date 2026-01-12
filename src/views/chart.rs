use crate::{
    app::{ClientGlobalState, METRICS_MAX_SIZE_PER_NODE},
    server_api::{get_settings, node_metrics},
    types::{METRIC_KEY_CPU_USAGE, METRIC_KEY_MEM_USED_MB, NodeId},
};

use super::icons::IconCancel;

use apexcharts_rs::prelude::ApexChart;
use chrono::Local;
use gloo_timers::future::TimeoutFuture;
use gloo_utils::format::JsValueSerdeExt;
use leptos::{logging, prelude::*};
use std::rc::Rc;
use wasm_bindgen::JsValue;

pub type ChartSeriesData = (Vec<(i64, f64)>, Vec<(i64, f64)>);

const CHART_MEM_SERIES_NAME: &str = "Memory (MB)";
const CHART_CPU_SERIES_NAME: &str = "CPU (%)";

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
    let chart_id = "metrics_chart".to_string();

    let metrics_chart_options = serde_json::json!(
        {
          "series": [],
          "noData": {
            "text": "Loading..."
          },
          "chart": {
            "id": chart_id,
            "width": "100%",
            "height": 380,
            "type": "line",
            "animations": {
              "enabled": true,
              "easing": "linear",
              "dynamicAnimation": {
                "speed": 1000
              }
            },
            "toolbar": {
              "show": false
            },
            "zoom": {
              "enabled": false
            }
          },
          "dataLabels": {
            "enabled": false
          },
          "colors": ["#F98080", "#3F83F8"],
          "stroke": {
            "curve": "smooth",
            "width": [3, 3]
          },
          "markers": {
            "size": 0
          },
          "xaxis": {
            "type": "datetime",
            "position": "bottom",
            "labels": {
              "show": true,
              "rotate": -30,
              "rotateAlways": false,
              "format": "HH:mm:ss",
              "style": {
                "colors": "#9CA3AF"
              }
            }
          },
          "yaxis": [
            {
              "labels": {
                "style": {
                  "colors": "#F98080"
                }
              },
              "title": {
                "text": CHART_MEM_SERIES_NAME,
                "style": {
                  "color": "#F98080"
                }
              }
            },
            {
              "opposite": true,
              "labels": {
                "style": {
                  "colors": "#3F83F8"
                }
              },
              "title": {
                "text": CHART_CPU_SERIES_NAME,
                "style": {
                  "color": "#3F83F8"
                }
              }
            }
          ],
          "legend": {
            "show": false
          },
          "tooltip": {
            "theme": "dark",
            "onDatasetHover": {
                "highlightDataSeries": true,
            }
          }
        }
    );

    let chart = RwSignal::new_local(None);

    let opts_clone = metrics_chart_options.clone();
    let chart_id_clone = chart_id.clone();
    Effect::new(move |_| {
        if !*is_render_chart.read() {
            return;
        }

        let opt = serde_json::to_string(&opts_clone).unwrap_or("".to_string());
        let c = ApexChart::new(&JsValue::from_str(&opt));
        c.render(&chart_id_clone);
        chart.update(|chart| *chart = Some(Rc::new(c)));
    });

    let opts_clone = metrics_chart_options.clone();
    Effect::new(move |_| {
        if !*is_render_chart.read() {
            return;
        }

        let mut opts_clone = opts_clone.clone();
        chart.with(|c| {
            if let Some(chart) = c {
                let (mem_data, cpu_data) = chart_data.get();
                opts_clone["series"] = serde_json::json!([
                    {
                      "name": CHART_MEM_SERIES_NAME,
                      "data": mem_data
                    },
                    {
                      "name": CHART_CPU_SERIES_NAME,
                      "data": cpu_data
                    }
                ]);
                match <JsValue as JsValueSerdeExt>::from_serde(&opts_clone) {
                    Ok(opt) => chart.update_options(&opt, Some(false), Some(true), Some(true)),
                    Err(err) => logging::log!("Failed to update chart: {err}"),
                }
            }
        });
    });

    view! { <div id=chart_id.clone()></div> }
}

// Fetch metrics data for a given node to render the charts
pub async fn node_metrics_update(
    node_id: NodeId,
    set_chart_data: WriteSignal<ChartSeriesData>,
) -> Result<(), ServerFnError> {
    logging::log!("Retriving node metrics from node {node_id}...");

    let polling_freq_millis =
        get_settings().await?.nodes_metrics_polling_freq.as_secs() as u32 * 2000;

    // hack to show timestamps with local timezone since apexcharts doesn't expose a way to do it
    let millis_offset = Local::now().offset().utc_minus_local() as i64 * 1_000;

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
                    m.extend(mem.iter().map(|v| {
                        (
                            v.timestamp - millis_offset,
                            v.value.parse::<f64>().unwrap_or_default(),
                        )
                    }));
                    c.extend(cpu.iter().map(|v| {
                        (
                            v.timestamp - millis_offset,
                            v.value.parse::<f64>().unwrap_or_default(),
                        )
                    }));

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
