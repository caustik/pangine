use pangine::Pangine;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "Usage: pangine-console [script.pae]";

#[derive(Debug, PartialEq, Eq)]
enum ConsoleMode {
    Interactive,
    Help,
    Script(PathBuf),
}

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pangine-console: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    match console_mode(args)? {
        ConsoleMode::Interactive => Pangine::new().debug_console().map_err(|error| error.to_string()),
        ConsoleMode::Help => {
            writeln!(io::stdout().lock(), "{USAGE}").map_err(|error| error.to_string())?;
            Ok(())
        }
        ConsoleMode::Script(path) => {
            let mut output = io::stdout().lock();
            Pangine::new().parse_script_file_with_details(&path, &mut output).map(|_| ()).map_err(|error| format!("failed to run {}: {error}", path.display()))
        }
    }
}

fn console_mode(args: impl IntoIterator<Item = OsString>) -> Result<ConsoleMode, String> {
    let mut args = args.into_iter();
    let Some(argument) = args.next() else {
        return Ok(ConsoleMode::Interactive);
    };
    if args.next().is_some() {
        return Err(format!("expected at most one script path\n{USAGE}"));
    }
    if argument == "-h" || argument == "--help" {
        return Ok(ConsoleMode::Help);
    }
    Ok(ConsoleMode::Script(argument.into()))
}

#[cfg(test)]
mod tests {
    use super::{console_mode, ConsoleMode};
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn no_argument_keeps_the_interactive_console() {
        assert_eq!(console_mode([]), Ok(ConsoleMode::Interactive));
    }

    #[test]
    fn one_path_runs_a_script() {
        let path = PathBuf::from("example.pae");
        assert_eq!(console_mode([path.clone().into_os_string()]), Ok(ConsoleMode::Script(path)));
    }

    #[test]
    fn help_and_extra_arguments_are_distinct() {
        assert_eq!(console_mode([OsString::from("--help")]), Ok(ConsoleMode::Help));
        assert!(console_mode([OsString::from("one"), OsString::from("two")]).is_err());
    }
}
