use super::{
    app::{
        ClientGlobalState, METRIC_KEY_CPU_USEAGE, METRIC_KEY_MEM_USED_MB, METRICS_MAX_SIZE_PER_NODE,
    },
    server_api::{get_settings, node_metrics},
    types::NodeId,
};

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
pub fn NodeChartView(
    is_render_chart: ReadSignal<bool>,
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
        .map(|id| id == node_id)
    {
        let update = node_metrics(node_id.clone(), since).await?;

        match (
            update.get(METRIC_KEY_MEM_USED_MB),
            update.get(METRIC_KEY_CPU_USEAGE),
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
