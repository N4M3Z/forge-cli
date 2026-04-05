mod assemble;
mod config;
mod copy;
mod deploy;
mod drift;
mod install;
mod output;
mod provenance;
mod release;
mod validate;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "forge", about = "Forge module toolkit", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Output results as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Assemble and deploy module content to provider directories
    Install {
        /// Path to the module root
        path: String,

        /// Deploy to a specific directory instead of default scope
        #[arg(long)]
        target: Option<String>,

        /// Overwrite user-modified files
        #[arg(long)]
        force: bool,

        /// Prompt before overwriting each file (not yet implemented, see CLI-0007)
        #[arg(long, short, hide = true)]
        interactive: bool,
    },

    /// Assemble module content into build/
    Assemble {
        /// Path to the module root
        path: String,
    },

    /// Deploy assembled files from build/ to provider directories
    Deploy {
        /// Path to the module root
        path: String,

        /// Deploy to a specific directory instead of default scope
        #[arg(long)]
        target: Option<String>,

        /// Overwrite user-modified files
        #[arg(long)]
        force: bool,

        /// Prompt before overwriting each file (not yet implemented, see CLI-0007)
        #[arg(long, short, hide = true)]
        interactive: bool,
    },

    /// Copy source files directly to a target directory (no assembly, no transforms)
    Copy {
        /// Path to the module root
        path: String,

        /// Target directory to copy into
        #[arg(long)]
        target: String,
    },

    /// Validate module files against schemas
    Validate {
        /// Path to the module root
        path: String,
    },

    /// Show provenance information for a deployed file or directory
    Provenance {
        /// Path to a deployed file or provider directory
        path: String,

        /// Filter by source module URI
        #[arg(long)]
        source: Option<String>,

        /// Show files without provenance
        #[arg(long)]
        show_orphans: bool,
    },

    /// Compare module content against an upstream reference
    Drift {
        /// Path to the module root (source)
        source: String,

        /// Path to the upstream reference module
        target: Option<String>,

        /// Comma-separated keys to ignore (use "body" to ignore body drift)
        #[arg(long, value_delimiter = ',')]
        ignore: Vec<String>,
    },

    /// Remove stale files from previous installs
    Clean {
        /// Path to the module root
        path: String,

        /// Clean a specific directory instead of default scope
        #[arg(long)]
        target: Option<String>,
    },

    /// Assemble and package module as release tarballs
    Release {
        /// Path to the module root
        path: String,

        /// Embed assets into the binary
        #[arg(long)]
        embed: bool,
    },
}

/// Parse CLI arguments, dispatch to subcommand, and return an exit code.
///
/// Exit codes: 0 = success, 1 = errors occurred, 2 = fatal error.
pub fn run() -> i32 {
    let args = Cli::parse();

    let (result, verb) = match args.command {
        Command::Install {
            path,
            target,
            force,
            interactive,
        } => (
            install::execute(&path, target.as_deref(), force, false, interactive),
            "deployed",
        ),
        Command::Assemble { path } => (assemble::execute(&path), "assembled"),
        Command::Deploy {
            path,
            target,
            force,
            interactive,
        } => (
            deploy::execute(&path, target.as_deref(), force, false, interactive),
            "deployed",
        ),
        Command::Copy { path, target } => (copy::execute(&path, &target), "copied"),
        Command::Validate { path } => (validate::execute(&path), "validated"),
        Command::Provenance {
            path,
            source,
            show_orphans,
        } => {
            return match provenance::execute(&path, source.as_deref(), show_orphans, args.json) {
                Ok(code) => code,
                Err(error) => {
                    eprintln!("fatal: {error}");
                    2
                }
            };
        }
        Command::Drift {
            source,
            target,
            ignore,
        } => {
            let upstream = target.as_deref().unwrap_or(".");
            return match drift::execute(&source, upstream, &ignore, args.json) {
                Ok(code) => code,
                Err(error) => {
                    eprintln!("fatal: {error}");
                    2
                }
            };
        }
        Command::Clean { path, target } => (
            deploy::execute(&path, target.as_deref(), false, true, false),
            "cleaned",
        ),
        Command::Release { path, embed } => (release::execute(&path, embed), "released"),
    };

    match result {
        Ok(action_result) => {
            output::print(&action_result, args.json, verb);
            i32::from(action_result.has_errors())
        }
        Err(error) => {
            eprintln!("fatal: {error}");
            2
        }
    }
}
