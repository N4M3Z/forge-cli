mod assemble;
mod config;
mod copy;
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

        /// Prompt before overwriting each file
        #[arg(long, short)]
        interactive: bool,
    },

    /// Assemble module content into build/
    Assemble {
        /// Path to the module root
        path: String,
    },

    /// Copy assembled files from build/ to provider directories
    Copy {
        /// Path to the module root
        path: String,

        /// Deploy to a specific directory instead of default scope
        #[arg(long)]
        target: Option<String>,

        /// Overwrite user-modified files
        #[arg(long)]
        force: bool,

        /// Prompt before overwriting each file
        #[arg(long, short)]
        interactive: bool,
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

    let result = match args.command {
        Command::Install {
            path,
            target,
            force,
            interactive,
        } => install::execute(&path, target.as_deref(), force, interactive),
        Command::Assemble { path } => assemble::execute(&path),
        Command::Copy {
            path,
            target,
            force,
            interactive,
        } => copy::execute(&path, target.as_deref(), force, interactive),
        Command::Validate { path } => validate::execute(&path),
        Command::Release { path, embed } => release::execute(&path, embed),
    };

    match result {
        Ok(action_result) => {
            print(&action_result, args.json);
            i32::from(action_result.has_errors())
        }
        Err(error) => {
            eprintln!("fatal: {error}");
            2
        }
    }
}

/// Print an `ActionResult` as human-readable text or JSON.
fn print(result: &ActionResult, json_output: bool) {
    if json_output {
        match serde_json::to_string_pretty(result) {
            Ok(json) => println!("{json}"),
            Err(err) => eprintln!("failed to serialize result: {err}"),
        }
        return;
    }

    for deployed in &result.installed {
        println!(
            "  installed: {} -> {} ({})",
            deployed.source, deployed.target, deployed.provider
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

    let installed_count = result.installed.len();
    let skipped_count = result.skipped.len();
    let error_count = result.errors.len();

    if installed_count > 0 || skipped_count > 0 || error_count > 0 {
        println!("  {installed_count} installed, {skipped_count} skipped, {error_count} errors");
    }
}
