use crate::cli_cmds::CliCommands;

use leptos::prelude::*;
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
                <span class="text-gray-900 dark:text-gray-200">
                    <pre>{format!("> {cmd}")}</pre>
                </span>
            }
            .into_any(),
            TerminalCmd::Output(res) => view! {
                <span class="">
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

    let handle_input = Action::new(move |cmd: &String| {
        let command = cmd.clone();
        history_cmds.update(|h| {
            h.push(cmd.clone());
            history_selected.set(h.len());
        });
        input_cmd.set(String::new());

        async move {
            let cmp = TerminalCmd::Input(format!("> {command}"));
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
        <div class="flex flex-col">
            <div class="w-full flex-1 overflow-hidden">
                <div class="p-2.5 border-transparent overflow-y-auto h-full">
                    <ul>
                        <For
                            each=move || output.read().clone().into_iter().enumerate()
                            key=|(i, _)| *i
                            let:child
                        >
                            <li>{child.1.into_view()}</li>
                        </For>
                    </ul>
                </div>
            </div>
            <div class="w-full flex-none h-2/10 md:flex md:items-center md:justify-between dark:bg-gray-700">
                <label for="terminal-prompt" class="mx-2 py-2 dark:bg-gray-700">
                    "#>"
                </label>
                <input
                    id="terminal-prompt"
                    class="w-full dark:bg-gray-700 dark:border-gray-700 dark:text-white dark:focus:ring-gray-700 dark:focus:border-gray-700"
                    type="text"
                    prop:value=move || input_cmd.get()
                    on:input=move |ev| { input_cmd.update(|i| *i = event_target_value(&ev)) }
                    on:keydown=move |ev| {
                        let cmd = input_cmd.get();
                        if ev.key() == "Enter" && !cmd.is_empty() {
                            handle_input.dispatch(cmd);
                        } else if ev.key() == "ArrowUp" {
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
            </div>
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
