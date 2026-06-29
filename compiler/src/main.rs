use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use x::{CompileOptions, build_with_options, check, emit_llvm_with_options, run_with_options};

fn main() -> ExitCode {
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        eprintln!("usage: x <check|build|run|emit-llvm> [file.x] [--target <triple>]");
        return ExitCode::from(2);
    }

    let command = args[1].as_str();
    let invocation = match Invocation::parse(&args[2..]) {
        Ok(invocation) => invocation,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("usage: x <check|build|run|emit-llvm> [file.x] [--target <triple>]");
            return ExitCode::from(2);
        }
    };
    let file = invocation.file;
    let options = invocation.options;
    let result = match command {
        "check" => check(&file).map(|_| println!("check ok: {}", file.display())),
        "build" => build_with_options(&file, &options)
            .map(|artifact| println!("built: {}", artifact.display())),
        "run" => run_with_options(&file, &options).map(|code| std::process::exit(code)),
        "emit-llvm" => emit_llvm_with_options(&file, &options).map(|llvm_ir| println!("{llvm_ir}")),
        _ => {
            eprintln!("unknown command `{command}`");
            eprintln!("usage: x <check|build|run|emit-llvm> [file.x] [--target <triple>]");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", err.render(&file));
            ExitCode::FAILURE
        }
    }
}

struct Invocation {
    file: PathBuf,
    options: CompileOptions,
}

impl Invocation {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = CompileOptions::from_env();
        let mut file = None;
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--target" => {
                    index += 1;
                    let Some(triple) = args.get(index) else {
                        return Err("missing value after `--target`".to_owned());
                    };
                    if triple.trim().is_empty() {
                        return Err("target triple cannot be empty".to_owned());
                    }
                    options.target_triple = Some(triple.clone());
                }
                value if value.starts_with("--target=") => {
                    let triple = value.trim_start_matches("--target=").trim();
                    if triple.is_empty() {
                        return Err("target triple cannot be empty".to_owned());
                    }
                    options.target_triple = Some(triple.to_owned());
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown option `{value}`"));
                }
                value => {
                    if file.is_some() {
                        return Err(format!("unexpected extra argument `{value}`"));
                    }
                    file = Some(PathBuf::from(value));
                }
            }
            index += 1;
        }
        Ok(Self {
            file: file.unwrap_or_else(|| PathBuf::from("main.x")),
            options,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn invocation_defaults_to_main_file() {
        let invocation = Invocation::parse(&[]).unwrap();
        assert_eq!(invocation.file, PathBuf::from("main.x"));
    }

    #[test]
    fn invocation_accepts_target_before_or_after_file() {
        let invocation =
            Invocation::parse(&args(&["--target", "wasm32-unknown-unknown", "main.x"])).unwrap();
        assert_eq!(invocation.file, PathBuf::from("main.x"));
        assert_eq!(
            invocation.options.target_triple.as_deref(),
            Some("wasm32-unknown-unknown")
        );

        let invocation =
            Invocation::parse(&args(&["main.x", "--target=x86_64-pc-windows-msvc"])).unwrap();
        assert_eq!(invocation.file, PathBuf::from("main.x"));
        assert_eq!(
            invocation.options.target_triple.as_deref(),
            Some("x86_64-pc-windows-msvc")
        );
    }

    #[test]
    fn invocation_rejects_bad_arguments() {
        for values in [
            vec!["--target"],
            vec!["--target="],
            vec!["--unknown"],
            vec!["a.x", "b.x"],
        ] {
            assert!(Invocation::parse(&args(&values)).is_err());
        }
    }
}
