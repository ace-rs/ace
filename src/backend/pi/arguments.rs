use std::path::PathBuf;

use super::super::{Invocation, PreflightError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputMode {
    Text,
    Json,
    Rpc,
}

pub(crate) struct Options {
    pub probe_command: Vec<String>,
    pub mcp_config: Option<PathBuf>,
    pub mode: OutputMode,
}

pub(super) fn parse(context: &Invocation) -> Result<Options, PreflightError> {
    let Some(program) = context
        .command
        .first()
        .filter(|program| !program.is_empty())
    else {
        return Err(PreflightError::InvalidArguments(
            "Pi command is empty; configure a Pi executable before launching ACE.".into(),
        ));
    };
    let mut options = Options {
        probe_command: vec![program.clone()],
        mcp_config: None,
        mode: OutputMode::Text,
    };
    let direct_pi = std::path::Path::new(program)
        .file_stem()
        .is_some_and(|name| name == "pi");
    let mut prefix = context.command[1..].iter();
    while let Some(argument) = prefix.next() {
        if is_config_flag(argument) {
            options.mcp_config = Some(config_path(argument, &mut prefix, context)?);
            continue;
        }
        if direct_pi
            || argument.starts_with('-')
            || matches!(
                argument.as_str(),
                "install" | "remove" | "uninstall" | "update" | "list" | "config" | "auth"
            )
        {
            return Err(PreflightError::InvalidArguments(format!(
                "Pi command prefix contains `{argument}`. Keep `cmd` as the executable and transparent wrapper prefix; use backend model/effort fields or ACE passthrough for Pi launch options."
            )));
        }
        options.probe_command.push(argument.clone());
    }

    let mut args = context.extra_args.iter();
    while let Some(argument) = args.next() {
        if argument == "--" {
            if args.any(|value| is_config_flag(value)) {
                return Err(ambiguous_selector());
            }
            break;
        }
        if is_config_flag(argument) {
            options.mcp_config = Some(config_path(argument, &mut args, context)?);
            continue;
        }
        match argument.as_str() {
            "--no-extensions" | "-ne" => {
                return Err(PreflightError::InvalidArguments(format!(
                    "Pi requires pi-mcp-adapter; remove `{argument}` from launch arguments. Use Pi's `config` command to inspect extension filtering."
                )));
            }
            "--mode" => options.mode = output_mode(args.next().map(String::as_str))?,
            "--use-theme" | "--tui-mode" => {
                let value = args.next().filter(|value| !value.starts_with('-'));
                let valid = value.is_some_and(|value| {
                    argument == "--use-theme" || matches!(value.as_str(), "regular" | "fullscreen")
                });
                if !valid {
                    return Err(PreflightError::InvalidArguments(format!(
                        "Pi {argument} requires {}.",
                        if argument == "--use-theme" {
                            "a theme name"
                        } else {
                            "regular or fullscreen"
                        }
                    )));
                }
            }
            value
                if takes_value(value)
                    && args
                        .next()
                        .is_some_and(|value| value == "--" || is_config_flag(value)) =>
            {
                return Err(ambiguous_selector());
            }
            _ => {}
        }
    }

    Ok(options)
}

fn ambiguous_selector() -> PreflightError {
    PreflightError::InvalidArguments(
        "Pi and pi-mcp-adapter interpret this argument boundary differently. Use --mcp-config only as a separate option before '--'; do not use '--' or a --mcp-config token as another option's value.".into(),
    )
}

fn is_config_flag(argument: &str) -> bool {
    argument == "--mcp-config" || argument.starts_with("--mcp-config=")
}

fn config_path(
    argument: &str,
    args: &mut std::slice::Iter<'_, String>,
    context: &Invocation,
) -> Result<PathBuf, PreflightError> {
    let value = if let Some(value) = argument.strip_prefix("--mcp-config=") {
        Some(value)
    } else {
        args.next()
            .map(String::as_str)
            .filter(|value| !value.starts_with(['-', '@']))
    };
    let value = value.filter(|value| !value.is_empty()).ok_or_else(|| PreflightError::InvalidArguments("--mcp-config requires a nonempty path; prefix paths beginning with '-' or '@' with './'.".into()))?;

    let mut path = PathBuf::new();
    for component in context.project_dir.join(value).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                path.pop();
            }
            component => path.push(component.as_os_str()),
        }
    }
    Ok(path)
}

fn output_mode(value: Option<&str>) -> Result<OutputMode, PreflightError> {
    match value {
        Some("text") => Ok(OutputMode::Text),
        Some("json") => Ok(OutputMode::Json),
        Some("rpc") => Ok(OutputMode::Rpc),
        _ => Err(PreflightError::InvalidArguments(
            "Pi --mode requires text, json, or rpc.".into(),
        )),
    }
}

fn takes_value(argument: &str) -> bool {
    matches!(
        argument,
        "--model"
            | "--provider"
            | "--thinking"
            | "--api-key"
            | "--system-prompt"
            | "--append-system-prompt"
            | "--extension"
            | "-e"
            | "--skill"
            | "--prompt-template"
            | "--theme"
            | "--export"
            | "--session"
            | "--session-id"
            | "--session-dir"
            | "--fork"
            | "--models"
            | "--tools"
            | "-t"
            | "--exclude-tools"
            | "-xt"
            | "--name"
            | "-n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Kind;
    use std::collections::HashMap;

    fn context(command: &[&str], args: &[&str]) -> Invocation {
        Invocation {
            kind: Kind::Pi,
            command: command.iter().map(|arg| (*arg).into()).collect(),
            project_dir: PathBuf::from("/project"),
            env: HashMap::new(),
            model: None,
            effort: None,
            extra_args: args.iter().map(|arg| (*arg).into()).collect(),
        }
    }

    #[test]
    fn direct_pi_cannot_receive_a_positional_prompt_during_probe() {
        assert!(parse(&context(&["pi", "hello"], &[])).is_err());
        assert!(parse(&context(&["/opt/bin/pi", "hello"], &[])).is_err());
    }

    #[test]
    fn rejects_ambiguous_adapter_selectors_consumed_by_native_options() {
        for flag in ["--name", "--export", "--model"] {
            for value in ["--mcp-config", "--mcp-config=other.json", "--"] {
                assert!(
                    parse(&context(&["pi"], &[flag, value, "other.json"])).is_err(),
                    "{flag} {value}"
                );
            }
        }
        assert!(parse(&context(&["pi"], &["--", "--mcp-config", "other.json"])).is_err());
    }

    #[test]
    fn validates_theme_and_tui_options_before_they_can_hide_flags() {
        for flag in ["--use-theme", "--tui-mode"] {
            assert!(parse(&context(&["pi"], &[flag, "--no-extensions"])).is_err());
            assert!(parse(&context(&["pi"], &[flag])).is_err());
        }
        assert!(parse(&context(&["pi"], &["--tui-mode", "invalid"])).is_err());
        assert!(
            parse(&context(
                &["pi"],
                &["--use-theme", "dark", "--tui-mode", "fullscreen"]
            ))
            .is_ok()
        );
    }

    #[test]
    fn adapter_selection_is_removed_only_from_the_probe_command() {
        let context = context(&["wrapper", "prefix", "--mcp-config", "instance.json"], &[]);
        let options = parse(&context).expect("known adapter option");
        assert_eq!(options.probe_command, ["wrapper", "prefix"]);
        assert_eq!(
            options.mcp_config,
            Some(PathBuf::from("/project/instance.json"))
        );
        assert_eq!(context.command[2], "--mcp-config");
    }

    #[test]
    fn adapter_selection_honors_equal_form_and_passthrough_precedence() {
        let options = parse(&context(
            &["pi", "--mcp-config=first.json"],
            &["--mcp-config", "last.json"],
        ))
        .expect("selected config");
        assert_eq!(
            options.mcp_config,
            Some(PathBuf::from("/project/last.json"))
        );
        assert_eq!(options.probe_command, ["pi"]);
    }

    #[test]
    fn malformed_adapter_selector_cannot_swallow_extension_disabling_flags() {
        for value in ["", "--no-extensions", "@attachment"] {
            assert!(
                parse(&context(&["pi"], &["--mcp-config", value])).is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn output_mode_is_the_last_native_mode_before_literal_arguments() {
        let options = parse(&context(
            &["pi"],
            &["--mode", "json", "--mode", "rpc", "--", "--mode", "text"],
        ))
        .expect("mode");
        assert_eq!(options.mode, OutputMode::Rpc);
        assert_eq!(
            parse(&context(&["pi"], &["--mode", "json"])).unwrap().mode,
            OutputMode::Json
        );
    }
}
