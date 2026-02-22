use crate::{
    server_api::{get_settings, test_llm_connection, update_settings},
    types::AppSettings,
};

use super::{
    helpers::show_error_alert_msg,
    icons::{
        IconBot, IconCheck, IconLayoutDashboard, IconLcdSettings, IconSave, IconServer, IconWallet,
    },
};

use alloy_primitives::Address;
use gloo_timers::future::TimeoutFuture;
use leptos::{logging, prelude::*};
use std::time::Duration;
use url::Url;
use wasm_bindgen_futures::spawn_local;

// Index used to keep track of currently active settings tab
const SETTINGS_TAB_NODE_MGMT: u8 = 0;
const SETTINGS_TAB_INTERFACE: u8 = 1;
const SETTINGS_TAB_REWARDS: u8 = 2;
const SETTINGS_TAB_LCD_DEVICE: u8 = 3;
const SETTINGS_TAB_AGENT: u8 = 4;

struct FormContent {
    saved_settings: RwSignal<AppSettings>,
    auto_upgrade: RwSignal<bool>,
    auto_upgrade_delay: RwSignal<Result<u64, (String, String)>>,
    bin_version_polling_freq: RwSignal<Result<u64, (String, String)>>,
    balances_retrieval_freq: RwSignal<Result<u64, (String, String)>>,
    metrics_polling_freq: RwSignal<Result<u64, (String, String)>>,
    disks_usage_check_freq: RwSignal<Result<u64, (String, String)>>,
    l2_network_rpc_url: RwSignal<Result<String, (String, String)>>,
    token_contract_address: RwSignal<Result<String, (String, String)>>,
    lcd_enabled: RwSignal<bool>,
    lcd_device: RwSignal<Result<String, (String, String)>>,
    lcd_addr: RwSignal<Result<String, (String, String)>>,
    node_list_page_size: RwSignal<Result<u64, (String, String)>>,
    node_list_mode: RwSignal<u64>,
    llm_base_url: RwSignal<Result<String, (String, String)>>,
    llm_model: RwSignal<Result<String, (String, String)>>,
    llm_api_key: RwSignal<String>,
    system_prompt: RwSignal<String>,
    max_context_messages: RwSignal<Result<u64, (String, String)>>,
    autonomous_enabled: RwSignal<bool>,
    autonomous_check_interval: RwSignal<Result<u64, (String, String)>>,
    autonomous_max_actions: RwSignal<Result<u64, (String, String)>>,
}

impl FormContent {
    pub fn new(settings: AppSettings) -> Self {
        Self {
            saved_settings: RwSignal::new(settings.clone()),
            auto_upgrade: RwSignal::new(settings.nodes_auto_upgrade),
            auto_upgrade_delay: RwSignal::new(Ok(settings.nodes_auto_upgrade_delay.as_secs())),
            bin_version_polling_freq: RwSignal::new(Ok(settings
                .node_bin_version_polling_freq
                .as_secs())),
            balances_retrieval_freq: RwSignal::new(Ok(settings
                .rewards_balances_retrieval_freq
                .as_secs())),
            metrics_polling_freq: RwSignal::new(Ok(settings.nodes_metrics_polling_freq.as_secs())),
            disks_usage_check_freq: RwSignal::new(Ok(settings.disks_usage_check_freq.as_secs())),
            l2_network_rpc_url: RwSignal::new(Ok(settings.l2_network_rpc_url.clone())),
            token_contract_address: RwSignal::new(Ok(settings.token_contract_address.clone())),
            lcd_enabled: RwSignal::new(settings.lcd_display_enabled),
            lcd_device: RwSignal::new(Ok(settings.lcd_device.clone())),
            lcd_addr: RwSignal::new(Ok(settings.lcd_addr.clone())),
            node_list_page_size: RwSignal::new(Ok(settings.node_list_page_size)),
            node_list_mode: RwSignal::new(settings.node_list_mode),
            llm_base_url: RwSignal::new(Ok(settings.llm_base_url)),
            llm_model: RwSignal::new(Ok(settings.llm_model)),
            llm_api_key: RwSignal::new(settings.llm_api_key),
            system_prompt: RwSignal::new(settings.system_prompt),
            max_context_messages: RwSignal::new(Ok(settings.max_context_messages)),
            autonomous_enabled: RwSignal::new(settings.autonomous_enabled),
            autonomous_check_interval: RwSignal::new(Ok(settings.autonomous_check_interval_secs)),
            autonomous_max_actions: RwSignal::new(Ok(settings.autonomous_max_actions_per_cycle)),
        }
    }

    pub fn is_unsaved_changes(&self) -> bool {
        let saved_settings = self.saved_settings.read();
        self.auto_upgrade.get() != saved_settings.nodes_auto_upgrade
            || self.auto_upgrade_delay.get()
                != Ok(saved_settings.nodes_auto_upgrade_delay.as_secs())
            || self.bin_version_polling_freq.get()
                != Ok(saved_settings.node_bin_version_polling_freq.as_secs())
            || self.balances_retrieval_freq.get()
                != Ok(saved_settings.rewards_balances_retrieval_freq.as_secs())
            || self.metrics_polling_freq.get()
                != Ok(saved_settings.nodes_metrics_polling_freq.as_secs())
            || self.disks_usage_check_freq.get()
                != Ok(saved_settings.disks_usage_check_freq.as_secs())
            || self.l2_network_rpc_url.get() != Ok(saved_settings.l2_network_rpc_url.clone())
            || self.token_contract_address.get()
                != Ok(saved_settings.token_contract_address.clone())
            || self.lcd_enabled.get() != saved_settings.lcd_display_enabled
            || self.lcd_device.get() != Ok(saved_settings.lcd_device.clone())
            || self.lcd_addr.get() != Ok(saved_settings.lcd_addr.clone())
            || self.node_list_page_size.get() != Ok(saved_settings.node_list_page_size)
            || self.node_list_mode.get() != saved_settings.node_list_mode
            || self.llm_base_url.get() != Ok(saved_settings.llm_base_url.clone())
            || self.llm_model.get() != Ok(saved_settings.llm_model.clone())
            || self.llm_api_key.get() != saved_settings.llm_api_key
            || self.system_prompt.get() != saved_settings.system_prompt
            || self.max_context_messages.get() != Ok(saved_settings.max_context_messages)
            || self.autonomous_enabled.get() != saved_settings.autonomous_enabled
            || self.autonomous_check_interval.get()
                != Ok(saved_settings.autonomous_check_interval_secs)
            || self.autonomous_max_actions.get()
                != Ok(saved_settings.autonomous_max_actions_per_cycle)
    }

    pub fn get_valid_changes(&self) -> Option<AppSettings> {
        let values = (
            self.auto_upgrade_delay.get(),
            self.bin_version_polling_freq.get(),
            self.balances_retrieval_freq.get(),
            self.metrics_polling_freq.get(),
            self.disks_usage_check_freq.get(),
            self.l2_network_rpc_url.get(),
            self.token_contract_address.get(),
            self.lcd_device.get(),
            self.lcd_addr.get(),
            self.node_list_page_size.get(),
            self.llm_base_url.get(),
            self.llm_model.get(),
            self.max_context_messages.get(),
            self.autonomous_check_interval.get(),
            self.autonomous_max_actions.get(),
        );
        if let (
            Ok(v1),
            Ok(v2),
            Ok(v3),
            Ok(v4),
            Ok(v5),
            Ok(v6),
            Ok(v7),
            Ok(v8),
            Ok(v9),
            Ok(v10),
            Ok(v11),
            Ok(v12),
            Ok(v13),
            Ok(v14),
            Ok(v15),
        ) = values
        {
            Some(AppSettings {
                nodes_auto_upgrade: self.auto_upgrade.get(),
                nodes_auto_upgrade_delay: Duration::from_secs(v1),
                node_bin_version_polling_freq: Duration::from_secs(v2),
                rewards_balances_retrieval_freq: Duration::from_secs(v3),
                nodes_metrics_polling_freq: Duration::from_secs(v4),
                disks_usage_check_freq: Duration::from_secs(v5),
                l2_network_rpc_url: v6,
                token_contract_address: v7,
                lcd_display_enabled: self.lcd_enabled.get(),
                lcd_device: v8,
                lcd_addr: v9,
                node_list_page_size: v10,
                node_list_mode: self.node_list_mode.get(),
                llm_base_url: v11,
                llm_model: v12,
                llm_api_key: self.llm_api_key.get(),
                system_prompt: self.system_prompt.get(),
                max_context_messages: v13,
                autonomous_enabled: self.autonomous_enabled.get(),
                autonomous_check_interval_secs: v14,
                autonomous_max_actions_per_cycle: v15,
            })
        } else {
            None
        }
    }

    pub fn discard_changes(&mut self) {
        let saved_settings = self.saved_settings.read();
        self.auto_upgrade.set(saved_settings.nodes_auto_upgrade);
        self.auto_upgrade_delay
            .set(Ok(saved_settings.nodes_auto_upgrade_delay.as_secs()));
        self.bin_version_polling_freq
            .set(Ok(saved_settings.node_bin_version_polling_freq.as_secs()));
        self.balances_retrieval_freq
            .set(Ok(saved_settings.rewards_balances_retrieval_freq.as_secs()));
        self.metrics_polling_freq
            .set(Ok(saved_settings.nodes_metrics_polling_freq.as_secs()));
        self.disks_usage_check_freq
            .set(Ok(saved_settings.disks_usage_check_freq.as_secs()));
        self.l2_network_rpc_url
            .set(Ok(saved_settings.l2_network_rpc_url.clone()));
        self.token_contract_address
            .set(Ok(saved_settings.token_contract_address.clone()));
        self.lcd_enabled.set(saved_settings.lcd_display_enabled);
        self.lcd_device.set(Ok(saved_settings.lcd_device.clone()));
        self.lcd_addr.set(Ok(saved_settings.lcd_addr.clone()));
        self.node_list_page_size
            .set(Ok(saved_settings.node_list_page_size));
        self.node_list_mode.set(saved_settings.node_list_mode);
        self.llm_base_url
            .set(Ok(saved_settings.llm_base_url.clone()));
        self.llm_model.set(Ok(saved_settings.llm_model.clone()));
        self.llm_api_key.set(saved_settings.llm_api_key.clone());
        self.system_prompt.set(saved_settings.system_prompt.clone());
        self.max_context_messages
            .set(Ok(saved_settings.max_context_messages));
        self.autonomous_enabled
            .set(saved_settings.autonomous_enabled);
        self.autonomous_check_interval
            .set(Ok(saved_settings.autonomous_check_interval_secs));
        self.autonomous_max_actions
            .set(Ok(saved_settings.autonomous_max_actions_per_cycle));
    }
}

#[component]
fn SettingsForm(form: RwSignal<FormContent>, active_tab: RwSignal<u8>) -> impl IntoView {
    // Status message for the "Test Connection" button
    let test_status = RwSignal::new(Option::<Result<String, String>>::None);
    let is_testing = RwSignal::new(false);

    view! {
        <span hidden=move || active_tab.read() != SETTINGS_TAB_NODE_MGMT>
            <SettingsCard
                icon=view! { <IconServer /> }.into_any()
                title="Nodes Management"
                description="Configure automated node upgrades, version checks, and metrics."
            >
                <SettingRow
                    label="Auto-Upgrade Nodes"
                    description="Automatically upgrade nodes to the latest version when available."
                >
                    <ToggleSwitch name="autoUpgrade" checked=form.read_untracked().auto_upgrade />
                </SettingRow>
                <SettingRow
                    label="Auto-Upgrade Delay"
                    description="Delay in seconds between when auto-upgrading each node."
                    error=Signal::derive(move || {
                        form.read().auto_upgrade_delay.read().clone().err()
                    })
                >
                    <NumberInput
                        name="upgradeDelay"
                        signal=form.read_untracked().auto_upgrade_delay
                        min=0
                    />
                </SettingRow>
                <SettingRow
                    label="Version Check Frequency"
                    description="How often (in seconds) to check for a new node binary version."
                    error=Signal::derive(move || {
                        form.read().bin_version_polling_freq.read().clone().err()
                    })
                >
                    <NumberInput
                        name="versionCheckFreq"
                        signal=form.read_untracked().bin_version_polling_freq
                        min=3600
                    />
                </SettingRow>
                <SettingRow
                    label="Metrics Fetch Frequency"
                    description="How often (in seconds) to fetch metrics from running nodes."
                    error=Signal::derive(move || {
                        form.read().metrics_polling_freq.read().clone().err()
                    })
                >
                    <NumberInput
                        name="metricsFreq"
                        signal=form.read_untracked().metrics_polling_freq
                        min=5
                    />
                </SettingRow>
                <SettingRow
                    label="Disks Usage Check Frequency"
                    description="How often (in seconds) to check the nodes disk usage."
                    error=Signal::derive(move || {
                        form.read().disks_usage_check_freq.read().clone().err()
                    })
                >
                    <NumberInput
                        name="metricsFreq"
                        signal=form.read_untracked().disks_usage_check_freq
                        min=10
                    />
                </SettingRow>
            </SettingsCard>
        </span>
        <span hidden=move || active_tab.read() != SETTINGS_TAB_INTERFACE>
            <SettingsCard
                icon=view! { <IconLayoutDashboard /> }.into_any()
                title="GUI Preferences"
                description="Choose preferred configurations for GUI."
            >
                <SettingRow
                    label="Default Node List View"
                    description="Choose the default layout for the Nodes list page."
                >
                    <SegmentedControl
                        signal=form.read_untracked().node_list_mode
                        options=vec!["Tile View".to_string(), "List View".to_string()]
                    />
                </SettingRow>
                <SettingRow
                    label="Nodes per Page"
                    description="The number of nodes to display per page in the list and tile views."
                    error=Signal::derive(move || {
                        form.read().node_list_page_size.read().clone().err()
                    })
                >
                    <NumberInput
                        name="nodeListPageSize"
                        signal=form.read_untracked().node_list_page_size
                        min=10
                    />
                </SettingRow>
            </SettingsCard>
        </span>
        <span hidden=move || active_tab.read() != SETTINGS_TAB_REWARDS>
            <SettingsCard
                icon=view! { <IconWallet /> }.into_any()
                title="Rewards"
                description="Manage settings related to L2 network connectivity and token rewards."
            >
                <SettingRow
                    label="Token Balance Query Frequency"
                    description="How often (in seconds) to query wallet balances from the L2 network."
                    error=Signal::derive(move || {
                        form.read().balances_retrieval_freq.read().clone().err()
                    })
                >
                    <NumberInput
                        name="tokenQueryFreq"
                        signal=form.read_untracked().balances_retrieval_freq
                        min=600
                    />
                </SettingRow>
                <SettingRow
                    label="L2 RPC URL"
                    description="The RPC endpoint used for querying balances and other on-chain data."
                    full_width=true
                    error=Signal::derive(move || {
                        form.read().l2_network_rpc_url.read().clone().err()
                    })
                >
                    <TextInputNew
                        name="rpcUrl"
                        signal=form.read_untracked().l2_network_rpc_url
                        validator=|v| { v.parse::<Url>().map_err(|err| err.to_string()).map(|_| v) }
                    />
                </SettingRow>
                <SettingRow
                    label="ERC20 Token Contract Address"
                    description="The smart contract address for the network's rewards token."
                    full_width=true
                    error=Signal::derive(move || {
                        form.read().token_contract_address.read().clone().err()
                    })
                >
                    <TextInputNew
                        name="erc20Address"
                        signal=form.read_untracked().token_contract_address
                        validator=|v| {
                            v.parse::<Address>().map_err(|err| err.to_string()).map(|_| v)
                        }
                    />
                </SettingRow>
            </SettingsCard>
        </span>
        <span hidden=move || active_tab.read() != SETTINGS_TAB_AGENT>
            <SettingsCard
                icon=view! { <IconBot class="w-6 h-6" /> }.into_any()
                title="AI Agent"
                description="Configure the local AI agent that can manage your nodes via natural language."
            >
                <SettingRow
                    label="LLM Base URL"
                    description="Base URL of your OpenAI-compatible LLM API (e.g. Ollama at http://localhost:11434)."
                    full_width=true
                    error=Signal::derive(move || form.read().llm_base_url.read().clone().err())
                >
                    <TextInputNew
                        name="llmBaseUrl"
                        signal=form.read_untracked().llm_base_url
                        validator=|v| {
                            v.parse::<url::Url>().map_err(|e| e.to_string()).map(|_| v)
                        }
                    />
                </SettingRow>
                <SettingRow
                    label="Model Name"
                    description="The model to use for chat and autonomous monitoring (e.g. llama3.2:3b, mistral)."
                    full_width=true
                    error=Signal::derive(move || form.read().llm_model.read().clone().err())
                >
                    <TextInputNew
                        name="llmModel"
                        signal=form.read_untracked().llm_model
                        validator=|v| {
                            if v.trim().is_empty() {
                                Err("LLM model is required.".to_string())
                            } else {
                                Ok(v)
                            }
                        }
                    />
                </SettingRow>
                <SettingRow
                    label="API Key"
                    description="Optional API key for authentication. Leave empty if your backend requires no key."
                    full_width=true
                    error=Signal::derive(|| None)
                >
                    <input
                        type="password"
                        class="w-full bg-slate-800 border border-slate-700 rounded-md px-3 py-2 text-sm focus:outline-none font-mono transition-colors focus:ring-1 focus:ring-indigo-500"
                        prop:value=move || form.read().llm_api_key.get()
                        on:input=move |ev| form.read().llm_api_key.set(event_target_value(&ev))
                        placeholder="(optional)"
                    />
                </SettingRow>
                <SettingRow
                    label="Custom System Prompt"
                    description="Optional instructions appended to the built-in Formicaio system prompt."
                    full_width=true
                    error=Signal::derive(|| None)
                >
                    <textarea
                        rows=3
                        class="w-full bg-slate-800 border border-slate-700 rounded-md px-3 py-2 text-sm focus:outline-none font-mono transition-colors focus:ring-1 focus:ring-indigo-500 resize-y"
                        prop:value=move || form.read().system_prompt.get()
                        on:input=move |ev| form.read().system_prompt.set(event_target_value(&ev))
                        placeholder="Additional instructions for the agent..."
                    />
                </SettingRow>
                <SettingRow
                    label="Max Context Messages"
                    description="How many prior chat messages to include in each LLM request."
                    error=Signal::derive(move || {
                        form.read().max_context_messages.read().clone().err()
                    })
                >
                    <NumberInput
                        name="maxContextMessages"
                        signal=form.read_untracked().max_context_messages
                        min=1
                    />
                </SettingRow>
                <SettingRow
                    label="Autonomous Check Interval (secs)"
                    description="How often the autonomous agent checks node health in the background."
                    error=Signal::derive(move || {
                        form.read().autonomous_check_interval.read().clone().err()
                    })
                >
                    <NumberInput
                        name="autonomousInterval"
                        signal=form.read_untracked().autonomous_check_interval
                        min=10
                    />
                </SettingRow>
                <SettingRow
                    label="Max Actions per Cycle"
                    description="Maximum number of tool-based actions the agent may take per monitoring cycle."
                    error=Signal::derive(move || {
                        form.read().autonomous_max_actions.read().clone().err()
                    })
                >
                    <NumberInput
                        name="autonomousMaxActions"
                        signal=form.read_untracked().autonomous_max_actions
                        min=1
                    />
                </SettingRow>
            </SettingsCard>
            // Test Connection + Save row
            <div class="mt-3 flex items-center gap-4 flex-wrap">
                <button
                    type="button"
                    prop:disabled=move || {
                        form.read().llm_base_url.read().is_err()
                            || form.read().llm_model.read().is_err()
                    }
                    on:click=move |_| {
                        if let Ok(base_url) = form.read_untracked().llm_base_url.get()
                            && let Ok(model) = form.read_untracked().llm_model.get()
                        {
                            let api_key = form.read_untracked().llm_api_key.get();
                            test_status.set(None);
                            is_testing.set(true);
                            spawn_local(async move {
                                let result = test_llm_connection(base_url, model, api_key).await;
                                test_status.set(Some(result.map_err(|e| e.to_string())));
                                is_testing.set(false);
                            });
                        }
                    }
                    class="px-4 py-2 text-sm font-bold bg-slate-800 hover:bg-slate-700 border border-slate-600 text-slate-200 rounded-lg transition-colors flex items-center gap-2 disabled:bg-slate-600 disabled:text-slate-400 disabled:opacity-75 disabled:shadow-none disabled:cursor-not-allowed"
                >
                    <Show when=move || is_testing.get()>
                        <span class="w-4 h-4 border-2 border-slate-400 border-t-transparent rounded-full animate-spin inline-block" />
                    </Show>
                    "Test Connection"
                </button>
                <Show when=move || {
                    test_status.read().is_some()
                }>
                    {move || {
                        match test_status.get() {
                            Some(Ok(model)) => {
                                view! {
                                    <span class="text-sm font-medium text-emerald-400">
                                        "Connected — model: " {model}
                                    </span>
                                }
                                    .into_any()
                            }
                            Some(Err(err)) => {
                                view! {
                                    <span class="text-sm font-medium text-rose-400">{err}</span>
                                }
                                    .into_any()
                            }
                            None => view! { <span /> }.into_any(),
                        }
                    }}
                </Show>
            </div>
        </span>
        <span hidden=move || active_tab.read() != SETTINGS_TAB_LCD_DEVICE>
            <SettingsCard
                icon=IconLcdSettings.into_any()
                title="LCD Device Setup"
                description="Configure an external I2C LCD display for real-time stats."
            >
                <SettingRow
                    label="Enable LCD Display"
                    description="Toggle on to send node statistics to a connected I2C LCD device."
                >
                    <ToggleSwitch name="lcdEnabled" checked=form.read_untracked().lcd_enabled />
                </SettingRow>
                <SettingRow
                    label="I2C Bus Number"
                    description="e.g., if the device path is /dev/i2c-1, the bus number is 1."
                    error=Signal::derive(move || form.read().lcd_device.read().clone().err())
                >
                    <TextInputNew
                        name="lcdBusNumber"
                        signal=form.read_untracked().lcd_device
                        validator=|v| { v.parse::<u8>().map_err(|err| err.to_string()).map(|_| v) }
                    />
                </SettingRow>
                <SettingRow
                    label="I2C Backpack Address"
                    description="The hexadecimal address of the I2C backpack (usually 0x27 or 0x3F)."
                    error=Signal::derive(move || form.read().lcd_addr.read().clone().err())
                >
                    <TextInputNew
                        name="lcdBackpackAddress"
                        signal=form.read_untracked().lcd_addr
                        validator=|v| {
                            u16::from_str_radix(v.strip_prefix("0x").unwrap_or(&v), 16)
                                .map_err(|err| err.to_string())
                                .map(|_| v)
                        }
                    />
                </SettingRow>
            </SettingsCard>
        </span>
    }
}

#[component]
pub fn SettingsView() -> impl IntoView {
    let current_settings = Resource::new(
        || (),
        |_| async move { get_settings().await.unwrap_or_default() },
    );
    let form_content = RwSignal::new(FormContent::new(AppSettings::default()));
    let active_tab = RwSignal::new(0);
    let is_saved = RwSignal::new(false);
    let is_dirty = move || form_content.read().is_unsaved_changes();

    #[cfg(feature = "lcd-disabled")]
    let lcd_disabled = true;
    #[cfg(not(feature = "lcd-disabled"))]
    let lcd_disabled = false;

    let update_settings_action = Action::new(move |settings: &AppSettings| {
        let settings_clone = settings.clone();
        async move {
            if let Err(err) = update_settings(settings_clone.clone()).await {
                let msg = format!("Failed to update settings: {err:?}");
                logging::log!("{msg}");
                show_error_alert_msg(msg);
            } else {
                is_saved.set(true);
                form_content.update(|f| f.saved_settings.set(settings_clone));
                spawn_local(async move {
                    TimeoutFuture::new(3000).await;
                    is_saved.set(false);
                });
            }
        }
    });

    view! {
        <div class="p-4 lg:p-8 animate-in fade-in slide-in-from-bottom-4 duration-500 relative">
            <form on:submit=move |e| {
                e.prevent_default();
                if let Some(valid_settings) = form_content.read_untracked().get_valid_changes() {
                    update_settings_action.dispatch(valid_settings);
                }
            }>
                <div class="grid grid-cols-1 lg:grid-cols-4 gap-8">
                    <nav class="lg:col-span-1 space-y-2">
                        <SideNavLink
                            icon=view! { <IconServer /> }.into_any()
                            label="Node Management"
                            active_tab
                            tab_index=SETTINGS_TAB_NODE_MGMT
                        />
                        <SideNavLink
                            icon=view! { <IconLayoutDashboard /> }.into_any()
                            label="GUI Preferences"
                            active_tab
                            tab_index=SETTINGS_TAB_INTERFACE
                        />
                        <SideNavLink
                            icon=view! { <IconWallet /> }.into_any()
                            label="Rewards"
                            active_tab
                            tab_index=SETTINGS_TAB_REWARDS
                        />
                        <SideNavLink
                            icon=view! { <IconBot class="w-5 h-5" /> }.into_any()
                            label="AI Agent"
                            active_tab
                            tab_index=SETTINGS_TAB_AGENT
                        />
                        <Show when=move || !lcd_disabled>
                            <SideNavLink
                                icon=IconLcdSettings.into_any()
                                label="LCD Device Setup"
                                active_tab
                                tab_index=SETTINGS_TAB_LCD_DEVICE
                            />
                        </Show>
                    </nav>
                    <main class="lg:col-span-3">
                        <Suspense fallback=move || {
                            view! { <p>"Loading..."</p> }
                        }>
                            {move || Suspend::new(async move {
                                form_content.set(FormContent::new(current_settings.await));
                                view! { <SettingsForm form=form_content active_tab /> }
                            })}
                        </Suspense>
                    </main>
                </div>

                <Show when=move || is_dirty()>
                    <div class="sticky bottom-6 mt-6 z-50 animate-in fade-in slide-in-from-bottom-4 duration-300">
                        <div class="max-w-4xl mx-auto bg-slate-900/80 backdrop-blur-lg border border-slate-700 rounded-2xl shadow-2xl p-4 flex items-center justify-between">
                            <p class="text-sm font-medium text-slate-300">
                                You have unsaved changes.
                            </p>
                            <div class="flex items-center gap-4">
                                <button
                                    type="button"
                                    on:click=move |_| form_content.update(|f| f.discard_changes())
                                    class="px-4 py-2 text-sm font-bold text-slate-400 hover:bg-slate-800 rounded-lg transition-colors"
                                >
                                    Discard
                                </button>
                                <button
                                    type="submit"
                                    class="bg-indigo-600 hover:bg-indigo-500 text-white px-5 py-2 rounded-lg font-bold transition-all shadow-lg shadow-indigo-500/20 flex items-center gap-2 disabled:bg-slate-600 disabled:text-slate-400 disabled:opacity-75 disabled:shadow-none disabled:cursor-not-allowed"
                                    prop:disabled=move || {
                                        form_content.read().get_valid_changes().is_none()
                                    }
                                >
                                    <IconSave />
                                    Save Changes
                                </button>
                            </div>
                        </div>
                    </div>
                </Show>

                <Show when=move || is_saved.get()>
                    <div class="fixed bottom-6 right-6 z-50 bg-emerald-500/90 backdrop-blur-lg border border-emerald-400/50 text-white px-5 py-3 rounded-2xl shadow-2xl flex items-center gap-3 animate-in fade-in slide-in-from-bottom-4 duration-300">
                        <IconCheck />
                        <span class="text-sm font-bold">Settings saved successfully!</span>
                    </div>
                </Show>
            </form>
        </div>
    }
}

#[component]
fn SideNavLink(
    icon: AnyView,
    label: &'static str,
    active_tab: RwSignal<u8>,
    tab_index: u8,
) -> impl IntoView {
    view! {
        <button
            type="button"
            on:click=move |_| active_tab.set(tab_index)
            class=move || {
                format!(
                    "w-full flex items-center gap-3 px-4 py-3 rounded-xl text-sm font-medium transition-all duration-200 {}",
                    if active_tab.get() == tab_index {
                        "bg-slate-800 text-white"
                    } else {
                        "text-slate-400 hover:text-slate-100 hover:bg-slate-800/50"
                    },
                )
            }
        >
            {icon}
            {label}
        </button>
    }
}

#[component]
fn SettingsCard(
    icon: AnyView,
    title: &'static str,
    description: &'static str,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="bg-slate-900 border border-slate-800 rounded-2xl shadow-xl animate-in fade-in duration-300">
            <header class="p-6 border-b border-slate-800">
                <div class="flex items-center gap-3 text-indigo-400 mb-1">
                    {icon} <h3 class="text-lg font-bold text-white">{title}</h3>
                </div>
                <p class="text-sm text-slate-400">{description}</p>
            </header>
            <div class="divide-y divide-slate-800">{children()}</div>
        </div>
    }
}

#[component]
fn SettingRow(
    label: &'static str,
    description: &'static str,
    #[prop(default = false)] full_width: bool,
    children: Children,
    #[prop(default = Signal::derive(|| None))] error: Signal<Option<(String, String)>>,
) -> impl IntoView {
    view! {
        <div class=format!(
            "p-6 {}",
            if full_width {
                ""
            } else {
                "flex flex-col md:flex-row items-start md:items-center justify-between gap-4"
            },
        )>
            <div class=if full_width { "mb-3" } else { "w-full md:w-3/5" }>
                <h4 class="font-semibold text-slate-200">{label}</h4>
                <p class="text-sm text-slate-500 mt-1">{description}</p>
            </div>
            <div class=if full_width {
                "w-full"
            } else {
                "w-full md:w-2/5"
            }>
                {children()} <Show when=move || error.read().is_some()>
                    <p class="text-sm text-rose-400 mt-2 animate-in fade-in duration-300">
                        {error.get().map(|(_, err)| err)}
                    </p>
                </Show>
            </div>
        </div>
    }
}

#[component]
pub fn NumberInput(
    signal: RwSignal<Result<u64, (String, String)>>,
    min: u64,
    name: &'static str,
) -> impl IntoView {
    let on_input = move |ev| {
        let orig_val = event_target_value(&ev);
        let val = match orig_val.parse::<u64>() {
            Ok(v) if v < min => Err((orig_val, format!("Value cannot be smaller than {min}."))),
            Ok(v) => Ok(v),
            Err(err) => Err((orig_val, format!("Invalid value, {err}"))),
        };
        signal.set(val);
    };

    view! {
        <input
            type="number"
            name=name
            value=signal.get_untracked().unwrap_or_default()
            prop:value=move || signal.get().map_or_else(|(v, _)| v, |v| v.to_string())
            on:input=on_input
            class=move || {
                format!(
                    "w-full bg-slate-800 border rounded-md px-3 py-2 text-sm focus:outline-none font-mono transition-colors {}",
                    if signal.read().is_err() {
                        "border-rose-500 ring-1 ring-rose-500/50"
                    } else {
                        "border-slate-700 focus:ring-1 focus:ring-indigo-500"
                    },
                )
            }
        />
    }
}

#[component]
pub fn TextInputNew(
    signal: RwSignal<Result<String, (String, String)>>,
    name: &'static str,
    validator: fn(String) -> Result<String, String>,
) -> impl IntoView {
    let on_input = move |ev| {
        let orig_value = event_target_value(&ev);
        let val = match validator(orig_value.clone()) {
            Ok(v) => Ok(v),
            Err(err) => Err((orig_value, format!("Invalid value, {err}"))),
        };
        signal.set(val);
    };

    view! {
        <input
            type="text"
            name=name
            value=signal.get_untracked().unwrap_or_default()
            prop:value=move || signal.get().map_or_else(|(v, _)| v, |v| v.to_string())
            on:input=on_input
            class=move || {
                format!(
                    "w-full bg-slate-800 border rounded-md px-3 py-2 text-sm focus:outline-none font-mono transition-colors {}",
                    if signal.read().is_err() {
                        "border-rose-500 ring-1 ring-rose-500/50"
                    } else {
                        "border-slate-700 focus:ring-1 focus:ring-indigo-500"
                    },
                )
            }
        />
    }
}

#[component]
fn ToggleSwitch(name: &'static str, checked: RwSignal<bool>) -> impl IntoView {
    view! {
        <label for=name class="flex items-center cursor-pointer">
            <div class="relative">
                <input
                    type="checkbox"
                    id=name
                    name=name
                    class="sr-only peer"
                    checked=move || checked.get()
                    on:change=move |_| checked.update(|v| *v = !*v)
                    prop:checked=move || checked.get()
                />
                <div class="w-11 h-6 bg-slate-700 rounded-full peer peer-checked:after:translate-x-full after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-indigo-600"></div>
            </div>
        </label>
    }
}

#[component]
fn SegmentedControl(signal: RwSignal<u64>, options: Vec<String>) -> impl IntoView {
    view! {
        <div class="flex items-center bg-slate-800 border border-slate-700 rounded-lg p-1 w-full md:w-auto">
            {options
                .into_iter()
                .enumerate()
                .map(|(index, opt)| {
                    view! {
                        <button
                            prop:key=opt
                            type="button"
                            on:click=move |_| signal.set(index as u64)
                            class=move || {
                                format!(
                                    "flex-1 px-4 py-1.5 rounded-md text-sm font-bold transition-all duration-200 {}",
                                    if signal.get() == index as u64 {
                                        "bg-indigo-600 text-white shadow-md"
                                    } else {
                                        "text-slate-400 hover:bg-slate-700"
                                    },
                                )
                            }
                        >
                            {opt.clone()}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}
