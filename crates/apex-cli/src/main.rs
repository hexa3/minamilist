use std::{env, ffi::OsString, path::PathBuf};

use anyhow::{Context, Result, bail};
use apex_core::Buffer;

fn main() -> Result<()> {
    match parse_args(env::args_os().skip(1))? {
        Action::Help => {
            print!("{}", help());
            Ok(())
        }
        Action::Version => {
            println!("minamilist {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Action::Launch(path) => {
            let (buffer, project_root) = launch_target(path)?;
            apex_ui::run(buffer, project_root)
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Action {
    Help,
    Version,
    Launch(Option<PathBuf>),
}

fn parse_args(arguments: impl IntoIterator<Item = OsString>) -> Result<Action> {
    let mut path = None;
    let mut positional_only = false;
    for argument in arguments {
        if !positional_only && argument == "--" {
            positional_only = true;
        } else if !positional_only && matches!(argument.to_str(), Some("-h" | "--help")) {
            return Ok(Action::Help);
        } else if !positional_only && matches!(argument.to_str(), Some("-V" | "--version")) {
            return Ok(Action::Version);
        } else if !positional_only && argument.to_string_lossy().starts_with('-') {
            bail!(
                "unknown option: {}\n\n{}",
                argument.to_string_lossy(),
                help()
            );
        } else if path.replace(PathBuf::from(argument)).is_some() {
            bail!("only one file or directory can be opened at a time");
        }
    }
    Ok(Action::Launch(path))
}

fn launch_target(path: Option<PathBuf>) -> Result<(Buffer, PathBuf)> {
    let current = env::current_dir().context("cannot read the current directory")?;
    let Some(path) = path else {
        return Ok((Buffer::new(), current));
    };
    let path = path
        .canonicalize()
        .with_context(|| format!("cannot open {}", path.display()))?;
    if path.is_dir() {
        Ok((Buffer::new(), path))
    } else {
        let root = path.parent().unwrap_or(&current).to_path_buf();
        Ok((Buffer::open(path)?, root))
    }
}

fn help() -> &'static str {
    "the minamilist IDE\n\nUsage: minamilist [OPTIONS] [FILE | DIRECTORY]\n\nOptions:\n  -h, --help       Print help\n  -V, --version    Print version\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_public_cli_surface() {
        assert_eq!(parse_args([]).unwrap(), Action::Launch(None));
        assert_eq!(parse_args(["--help".into()]).unwrap(), Action::Help);
        assert_eq!(
            parse_args(["--".into(), "-draft.rs".into()]).unwrap(),
            Action::Launch(Some("-draft.rs".into()))
        );
        assert!(parse_args(["--wat".into()]).is_err());
    }
}
