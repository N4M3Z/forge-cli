mod assemble;
mod config;
mod copy;
mod deploy;
mod install;
mod release;
mod validate;

use clap::{Parser, Subcommand};
use commands::result::ActionResult;

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
            install::execute(&path, target.as_deref(), force, interactive),
            "deployed",
        ),
        Command::Assemble { path } => (assemble::execute(&path), "assembled"),
        Command::Deploy {
            path,
            target,
            force,
            interactive,
        } => (
            deploy::execute(&path, target.as_deref(), force, interactive),
            "deployed",
        ),
        Command::Copy { path, target } => (copy::execute(&path, &target), "copied"),
        Command::Validate { path } => (validate::execute(&path), "validated"),
        Command::Release { path, embed } => (release::execute(&path, embed), "released"),
    };

    match result {
        Ok(action_result) => {
            print(&action_result, args.json, verb);
            i32::from(action_result.has_errors())
        }
        Err(error) => {
            eprintln!("fatal: {error}");
            2
        }
    }
}

/// Print an `ActionResult` as human-readable text or JSON.
fn print(result: &ActionResult, json_output: bool, verb: &str) {
    if json_output {
        match serde_json::to_string_pretty(result) {
            Ok(json) => println!("{json}"),
            Err(err) => eprintln!("failed to serialize result: {err}"),
        }
        return;
    }

    for entry in &result.installed {
        println!(
            "  {verb}: {} -> {} ({})",
            entry.source, entry.target, entry.provider
        );
    }

    for skipped in &result.skipped {
        println!(
            "  skipped: {} ({}, {:?})",
            skipped.target, skipped.provider, skipped.reason
        );
    }

    for error in &result.errors {
        eprintln!("  error: {error}");
    }

    let action_count = result.installed.len();
    let skipped_count = result.skipped.len();
    let error_count = result.errors.len();

    if action_count > 0 || skipped_count > 0 || error_count > 0 {
        println!("  {action_count} {verb}, {skipped_count} skipped, {error_count} errors");
    }
}
