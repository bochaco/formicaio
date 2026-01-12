use super::icons::{IconPrompt, IconTerminal};
use crate::cli_cmds::CliCommands;

use leptos::{html, prelude::*};
use std::{cmp::Ordering, io::Cursor};
use structopt::StructOpt;

// Type of command to display in the terminal
#[derive(Clone)]
enum TerminalCmd {
    Input(String),
    Output(String),
    Error(String),
}

impl TerminalCmd {
    fn into_view(self) -> AnyView {
        match self {
            TerminalCmd::Input(cmd) => view! {
                <span class="text-indigo-400 mt-2">
                    <pre>{format!("> {cmd}")}</pre>
                </span>
            }
            .into_any(),
            TerminalCmd::Output(res) => view! {
                <span class="text-slate-300 italic py-2">
                    <pre>{res}</pre>
                </span>
            }
            .into_any(),
            TerminalCmd::Error(err) => view! {
                <span class="text-red-700 dark:text-red-400">
                    <pre>{format!("{err}\n")}</pre>
                </span>
            }
            .into_any(),
        }
    }
}

#[component]
pub fn TerminalView() -> impl IntoView {
    let output = RwSignal::new(vec![]);
    let input_cmd = RwSignal::new(String::new());
    let history_cmds = RwSignal::new(vec![]);
    let history_selected = RwSignal::new(0);
    let scroll_ref: NodeRef<html::Main> = NodeRef::new();

    Effect::new(move |_| {
        if output.read().is_empty() {
            return;
        }
        if let Some(node) = scroll_ref.get() {
            node.set_scroll_top(node.scroll_height());
        }
    });

    let handle_input = Action::new(move |cmd: &String| {
        let command = cmd.clone();
        history_cmds.update(|h| {
            h.push(cmd.clone());
            history_selected.set(h.len());
        });
        input_cmd.set(String::new());

        async move {
            let cmp = TerminalCmd::Input(command.clone());
            output.update(|o| o.push(cmp));

            match process_command(&command).await {
                Ok(res) => {
                    let cmp = TerminalCmd::Output(res.to_string());
                    output.update(|o| o.push(cmp));
                }
                Err(err) => {
                    let cmp = TerminalCmd::Error(format!("{err}\n"));
                    output.update(|o| o.push(cmp));
                }
            }
        }
    });
    handle_input.dispatch("--help".to_string());

    view! {
        <div class="flex flex-col h-full bg-slate-900 border border-slate-800 rounded-2xl overflow-hidden shadow-2xl animate-in fade-in duration-500">
            <div class="bg-slate-800 px-4 py-3 flex items-center justify-between border-b border-slate-700">
                <div class="flex gap-1.5">
                    <div class="w-3 h-3 rounded-full bg-rose-500" />
                    <div class="w-3 h-3 rounded-full bg-amber-500" />
                    <div class="w-3 h-3 rounded-full bg-emerald-500" />
                </div>
            </div>

            <main
                node_ref=scroll_ref
                class="flex-1 p-6 font-mono text-sm overflow-y-auto bg-slate-950/90 no-scrollbar space-y-1.5"
            >
                <div class="text-emerald-500 mb-4 flex items-center gap-2">
                    <div class="flex items-center gap-2 text-xs font-mono text-slate-400">
                        <IconTerminal />
                        Welcome to Formicaio Terminal
                    </div>
                </div>
                <ul>
                    <For
                        each=move || output.read().clone().into_iter().enumerate()
                        key=|(i, _)| *i
                        let:child
                    >
                        <li>{child.1.into_view()}</li>
                    </For>
                </ul>
            </main>

            <form
                on:submit=move |ev| {
                    ev.prevent_default();
                    let cmd = input_cmd.get();
                    if !cmd.is_empty() {
                        handle_input.dispatch(cmd);
                    }
                }
                class="p-4 bg-slate-950 border-t border-slate-800 flex items-center gap-3"
            >
                <IconPrompt />
                <input
                    id="terminal-prompt"
                    type="text"
                    autofocus
                    prop:autocomplete="off"
                    placeholder="Type command here..."
                    class="flex-1 bg-transparent border-none text-slate-100 font-mono text-sm focus:outline-none placeholder-slate-700"
                    prop:value=move || input_cmd.get()
                    on:input=move |ev| input_cmd.update(|i| *i = event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "ArrowUp" {
                            history_selected
                                .update(|s| {
                                    if *s > 0 {
                                        *s -= 1;
                                        input_cmd.update(|i| *i = history_cmds.get()[*s].clone());
                                    }
                                });
                        } else if ev.key() == "ArrowDown" {
                            history_selected
                                .update(|s| {
                                    let history_cmds = history_cmds.get();
                                    match (*s).cmp(&(history_cmds.len() - 1)) {
                                        Ordering::Less => {
                                            *s += 1;
                                            input_cmd.update(|i| *i = history_cmds[*s].clone());
                                        }
                                        Ordering::Equal => {
                                            *s += 1;
                                            input_cmd.update(|i| *i = String::new());
                                        }
                                        Ordering::Greater => {}
                                    }
                                });
                        }
                    }
                />
            </form>

        </div>
    }
}

async fn process_command(cmd: &str) -> Result<String, String> {
    let mut args = if cmd.starts_with("formicaio") {
        vec![]
    } else {
        vec!["formicaio"]
    };
    args.extend(cmd.split(" "));
    match CliCommands::from_iter_safe(args.iter().filter(|arg| !arg.is_empty())) {
        Ok(cmd) => {
            let response = cmd
                .process_command()
                .await
                .map_err(|err| format!("{err}\n"))?;
            let mut output = Vec::new();
            let mut cursor = Cursor::new(&mut output);
            response.print(&mut cursor).unwrap();

            Ok(String::from_utf8_lossy(&output).to_string())
        }
        Err(err) => Err(err.to_string()),
    }
}
