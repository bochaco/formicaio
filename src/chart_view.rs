use super::{
    app::{ClientGlobalState, METRIC_KEY_CPU_USEAGE, METRIC_KEY_MEM_USED_MB},
    server_api::node_metrics,
};

use apexcharts_rs::prelude::{ApexChart, ChartSeries, ChartType, SeriesData};
use gloo_timers::future::TimeoutFuture;
use gloo_utils::format::JsValueSerdeExt;
use leptos::*;
use serde_json::Value;
use wasm_bindgen::JsValue;

pub type ChartSeriesData = (Vec<(i64, i64)>, Vec<(i64, i64)>);

#[component]
pub fn NodeChartView(chart_data: ReadSignal<ChartSeriesData>) -> impl IntoView {
    let chart_id = "metrics_chart".to_string();
    let mem_serie = ChartSeries {
        name: "Memory".to_string(),
        data: SeriesData::Timestamped(vec![]),
        color: "#F98080".to_string(),
        r#type: None,
        z_index: None,
    };
    let cpu_serie = ChartSeries {
        name: "CPU".to_string(),
        data: SeriesData::Timestamped(vec![]),
        color: "#3F83F8".to_string(),
        r#type: None,
        z_index: None,
    };

    let metrics_chart_options = format!(
        r##"{{
          "series": [],
          "noData": {{
            "text": "Loading..."
          }},
          "chart": {{
            "id": "{chart_id}",
            "width": "100%",
            "height": 350,
            "type": "line",
            "animations": {{
              "enabled": true,
              "easing": "linear",
              "dynamicAnimation": {{
                "speed": 1000
              }}
            }},
            "toolbar": {{
              "show": false
            }},
            "zoom": {{
              "enabled": false
            }}
          }},
          "dataLabels": {{
            "enabled": false
          }},
          "colors": ["#F98080", "#3F83F8"],
          "stroke": {{
            "curve": "smooth"
          }},
          "markers": {{
            "size": 0
          }},
          "xaxis": {{
            "type": "datetime",
            "position": "bottom",
            "labels": {{
              "show": true,
              "format": "dd/MMM H:m:s"
            }}
          }},
          "yaxis": [
            {{
              "labels": {{
                "style": {{
                  "colors": "#F98080"
                }}
              }},
              "title": {{
                "text": "Memory (MB)",
                "style": {{
                  "color": "#F98080"
                }}
              }}
            }},
            {{
              "opposite": true,
              "labels": {{
                "style": {{
                  "colors": "#3F83F8"
                }}
              }},
              "title": {{
                "text": "CPU (%)",
                "style": {{
                  "color": "#3F83F8"
                }}
              }}
            }}
          ],
          "legend": {{
            "show": false
          }}
        }}"##
    );

    let chart = create_rw_signal(None);

    let mut options = serde_json::from_str::<Value>(&metrics_chart_options)
        .unwrap_or_else(|_| panic!("Invalid JSON: {}", metrics_chart_options));
    options["chart"]["type"] = Value::String(ChartType::Line.to_string());

    let opts_clone = options.clone();
    let chart_id_clone = chart_id.clone();
    create_effect(move |_| {
        let opt = serde_json::to_string(&opts_clone).unwrap_or("".to_string());
        let c = ApexChart::new(&JsValue::from_str(&opt));
        c.render(&chart_id_clone);
        chart.set(Some(c));
    });

    let opts_clone = options.clone();
    create_effect(move |_| {
        let mut opts_clone = opts_clone.clone();
        chart.with(|c| {
            if let Some(chart) = c {
                let mut new_mem_series = mem_serie.clone();
                let mut new_cpu_series = cpu_serie.clone();
                let (mem_data, cpu_data) = chart_data.get();
                new_mem_series.data = SeriesData::Timestamped(mem_data);
                new_cpu_series.data = SeriesData::Timestamped(cpu_data);
                opts_clone["series"] = serde_json::to_value(&[new_mem_series, new_cpu_series])
                    .unwrap_or(Value::Array(vec![]));
                let opt = <JsValue as JsValueSerdeExt>::from_serde(&opts_clone).unwrap();
                chart.update_options(&opt, Some(false), Some(true), Some(true));
            }
        });
    });

    view! { <div id=chart_id.clone()></div> }
}

// Fetch metrics data for a given node to render the charts
pub async fn node_metrics_update(
    container_id: String,
    set_chart_data: WriteSignal<ChartSeriesData>,
) -> Result<(), ServerFnError> {
    logging::log!("Retriving node metrics from container {container_id}...");

    // use context to check if we should stop retrieving the metrics
    let context = expect_context::<ClientGlobalState>();
    let mut since = None;
    set_chart_data.set((vec![], vec![]));

    loop {
        if !context.metrics_update_is_on.get_untracked() {
            break;
        }

        let update = node_metrics(
            container_id.clone(),
            since,
            vec![
                METRIC_KEY_MEM_USED_MB.to_string(),
                METRIC_KEY_CPU_USEAGE.to_string(),
            ],
        )
        .await?;

        if let Some(values) = update.get(METRIC_KEY_MEM_USED_MB) {
            if !values.is_empty() {
                since = values.last().map(|m| m.timestamp);
                set_chart_data.update(|(mem, _)| {
                    mem.extend(
                        values
                            .iter()
                            .map(|v| (v.timestamp, v.value.parse::<i64>().unwrap())),
                    )
                });
            }
        }

        if let Some(values) = update.get(METRIC_KEY_CPU_USEAGE) {
            if !values.is_empty() {
                set_chart_data.update(|(_, cpu)| {
                    cpu.extend(
                        values
                            .iter()
                            .map(|v| (v.timestamp, v.value.parse::<i64>().unwrap())),
                    )
                });
            }
        }

        // FIXME: shortcircuit the delay if the flag is set to off
        TimeoutFuture::new(4000).await;
    }

    logging::log!("Node metrics update from container {container_id} stopped.");
    Ok(())
}