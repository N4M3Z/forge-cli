mod assemble;
mod config;
mod copy;
#[cfg(feature = "dashboard")]
mod dashboard;
mod deploy;
mod dotforge;
mod drift;
mod init;
mod install;
mod output;
mod provenance;
mod release;
mod validate;
mod watchlist;

#[cfg(test)]
mod tests;

use clap::{Parser, Subcommand};
use commands::error::Error;
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
    /// Initialize a new forge module with required files and schemas
    Init {
        /// Directory to scaffold the new module into (created if missing).
        #[arg(long, value_name = "DIR")]
        target: String,
    },

    /// Assemble and deploy module content to provider directories
    #[command(after_help = "EXAMPLES:\n  \
        # Install the current directory's module for all providers under ~/\n  \
        cd ~/Modules/forge-core && forge install --target ~\n  \
        \n  \
        # Install a specific module for opencode only\n  \
        forge install --source ~/Modules/forge-core --target ~ --provider opencode\n\n\
        TARGET LAYOUT:\n  \
        --target <DIR> deploys each provider to <DIR>/<provider-target>:\n    \
        claude   → <DIR>/.claude\n    \
        codex    → <DIR>/.codex\n    \
        gemini   → <DIR>/.gemini\n    \
        opencode → <DIR>/.opencode\n  \
        Without --target, providers deploy under the current directory. \
        In consumer mode (.forge present at --source), --target defaults to --source.")]
    Install {
        /// Module root to install from (must contain module.yaml). Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Base directory under which each provider gets its own subdirectory.
        /// Without this flag, providers deploy under the current directory.
        /// In consumer mode (.forge present at --source), this defaults to --source.
        #[arg(long, value_name = "DIR")]
        target: Option<String>,

        /// Deploy only the named provider(s). Repeatable.
        /// Available: claude, codex, gemini, opencode.
        #[arg(long, value_name = "NAME")]
        provider: Vec<String>,

        /// Overwrite user-modified files
        #[arg(long)]
        force: bool,

        /// Prompt before overwriting each file (not yet implemented, see CLI-0007)
        #[arg(long, short, hide = true)]
        interactive: bool,

        /// Skip pruning deployed files absent from source. By default, install
        /// prunes stale agents/skills/rules and quarantines them to
        /// <target>/.trash/<UTC-ts>/ for recoverability.
        #[arg(long)]
        no_prune: bool,

        /// Show what would be pruned without moving files.
        #[arg(long)]
        dry_run: bool,
    },

    /// Assemble module content into build/
    Assemble {
        /// Module root to assemble (must contain module.yaml). Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Deploy assembled files from build/ to provider directories
    Deploy {
        /// Module root containing build/ to deploy from. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Base directory under which each provider gets its own subdirectory.
        /// Without this flag, providers deploy under the current directory.
        /// In consumer mode (.forge present at --source), this defaults to --source.
        #[arg(long, value_name = "DIR")]
        target: Option<String>,

        /// Deploy only the named provider(s). Repeatable.
        /// Available: claude, codex, gemini, opencode.
        #[arg(long, value_name = "NAME")]
        provider: Vec<String>,

        /// Overwrite user-modified files
        #[arg(long)]
        force: bool,

        /// Prompt before overwriting each file (not yet implemented, see CLI-0007)
        #[arg(long, short, hide = true)]
        interactive: bool,

        /// Skip pruning deployed files absent from source. By default, deploy
        /// prunes stale agents/skills/rules and quarantines them to
        /// <target>/.trash/<UTC-ts>/ for recoverability.
        #[arg(long)]
        no_prune: bool,

        /// Show what would be pruned without moving files.
        #[arg(long)]
        dry_run: bool,
    },

    /// Copy source files directly to a target directory (no assembly, no transforms)
    Copy {
        /// Module root to copy from.
        #[arg(long, value_name = "DIR")]
        source: String,

        /// Directory to copy into.
        #[arg(long, value_name = "DIR")]
        target: String,

        /// Skip SLSA provenance sidecar generation
        #[arg(long)]
        skip_provenance: bool,
    },

    /// Validate module files against schemas
    Validate {
        /// Module root to validate (must contain module.yaml). Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,
    },

    /// Show provenance information for a deployed file or directory
    Provenance {
        /// Deployed file or provider directory to inspect. Defaults to `.`.
        #[arg(long, value_name = "DIR_OR_FILE", default_value = ".")]
        target: String,

        /// Filter by source module URI (e.g. <https://github.com/...>)
        #[arg(long, value_name = "URI")]
        source_uri: Option<String>,

        /// Show files without provenance
        #[arg(long)]
        show_orphans: bool,
    },

    /// Compare module content against an upstream reference
    Drift {
        /// Module root to compare. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Upstream reference module to compare against.
        #[arg(long, value_name = "DIR")]
        upstream: String,

        /// Comma-separated keys to ignore (use "body" to ignore body drift)
        #[arg(long, value_delimiter = ',')]
        ignore: Vec<String>,
    },

    /// Remove stale files from previous installs
    Clean {
        /// Module root whose manifests drive the cleanup. Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Base directory under which each provider's files were deployed.
        /// Without this flag, the current directory is used.
        #[arg(long, value_name = "DIR")]
        target: Option<String>,
    },

    /// Launch a read-only web dashboard showing artifact state, provenance,
    /// and deployment status across all providers
    #[cfg(feature = "dashboard")]
    Dashboard {
        /// Base directory to scan for modules (one level deep). Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        root: String,

        /// Port to bind. Defaults to 40000, falling back to 40001 if busy.
        #[arg(long, value_name = "PORT")]
        port: Option<u16>,
    },

    /// Assemble and package module as release tarballs
    Release {
        /// Module root to package (must contain module.yaml). Defaults to `.`.
        #[arg(long, value_name = "DIR", default_value = ".")]
        source: String,

        /// Embed assets into the binary
        #[arg(long)]
        embed: bool,
    },

    /// Manage the watchlist of module and deployment locations to monitor
    Watch {
        #[command(subcommand)]
        action: WatchAction,
    },
}

#[derive(Subcommand)]
enum WatchAction {
    /// List watched locations
    List,
    /// Add a local path to the watchlist
    Add {
        /// Path to a module or deployment target (supports a leading `~/`).
        path: String,
    },
    /// Watch a remote repo pinned to a commit SHA
    Git {
        /// HTTPS URL of the repo to monitor.
        url: String,
        /// Full 40-char lowercase-hex commit SHA to pin.
        #[arg(long = "ref")]
        reference: String,
    },
    /// Remove a watched entry by its path or git URL
    Remove {
        /// Path or git URL to stop watching.
        path: String,
    },
}

/// Parse CLI arguments, dispatch to subcommand, and return an exit code.
///
/// Exit codes: 0 = success, 1 = errors occurred, 2 = fatal error.
pub fn run() -> i32 {
    let args = Cli::parse();

    let (result, verb) = match args.command {
        Command::Init { target } => (init::execute(&target), "initialized"),
        Command::Install {
            source,
            target,
            provider,
            force,
            interactive,
            no_prune,
            dry_run,
        } => (
            install::execute(
                &source,
                target.as_deref(),
                &provider,
                force,
                !no_prune,
                interactive,
                dry_run,
            ),
            "deployed",
        ),
        Command::Assemble { source } => (assemble::execute(&source), "assembled"),
        Command::Deploy {
            source,
            target,
            provider,
            force,
            interactive,
            no_prune,
            dry_run,
        } => (
            deploy::execute(
                &source,
                target.as_deref(),
                &provider,
                force,
                !no_prune,
                interactive,
                dry_run,
            ),
            "deployed",
        ),
        Command::Copy {
            source,
            target,
            skip_provenance,
        } => (copy::execute(&source, &target, skip_provenance), "copied"),
        Command::Validate { source } => (validate::execute(&source), "validated"),
        Command::Provenance {
            target,
            source_uri,
            show_orphans,
        } => {
            return exit_code(provenance::execute(
                &target,
                source_uri.as_deref(),
                show_orphans,
                args.json,
            ));
        }
        Command::Drift {
            source,
            upstream,
            ignore,
        } => return exit_code(drift::execute(&source, &upstream, &ignore, args.json)),
        Command::Clean { source, target } => (
            deploy::execute(&source, target.as_deref(), &[], false, true, false, false),
            "cleaned",
        ),
        #[cfg(feature = "dashboard")]
        Command::Dashboard { root, port } => return exit_code(dashboard::execute(&root, port)),
        Command::Release { source, embed } => (release::execute(&source, embed), "released"),
        Command::Watch { action } => return run_watch(action, args.json),
    };

    report(result, args.json, verb)
}

/// Collapse a subcommand's `Result<exit_code, _>` into a process exit code,
/// printing a `fatal:` line on `Err`.
fn exit_code<E: std::fmt::Display>(result: Result<i32, E>) -> i32 {
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("fatal: {error}");
            2
        }
    }
}

/// Dispatch a `forge watch` subcommand to its handler.
fn run_watch(action: WatchAction, json: bool) -> i32 {
    let result = match action {
        WatchAction::List => watchlist::list(json),
        WatchAction::Add { path } => watchlist::add_path(&path, json),
        WatchAction::Git { url, reference } => watchlist::add_git(&url, &reference, json),
        WatchAction::Remove { path } => watchlist::remove(&path, json),
    };
    exit_code(result)
}

/// Print a structured `ActionResult` and return the corresponding exit code.
fn report(result: Result<ActionResult, Error>, json: bool, verb: &str) -> i32 {
    match result {
        Ok(action_result) => {
            output::print(&action_result, json, verb);
            i32::from(action_result.has_errors())
        }
        Err(error) => {
            eprintln!("fatal: {error}");
            2
        }
    }
}
