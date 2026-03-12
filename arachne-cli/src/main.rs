use std::{fs, path::PathBuf, process::ExitCode, time::Instant};

use anyhow::{Result, anyhow};
use arachne_codegen::{Config, generate_with_report};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use colored::Colorize;
use ecore_rs::ctx::Ctx;

#[derive(Debug, Parser)]
#[command(
    name = "arachne",
    version,
    about = "Arachne CLI: parse Ecore and generate Rust CRDT projects"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Parse an Ecore file and print its internal representation
    #[command(name = "parse", alias = "ecore-parse")]
    Parse(ParseArgs),
    /// Generate a Rust CRDT project from an Ecore metamodel
    #[command(name = "generate", alias = "gen")]
    Generate(GenerateArgs),
}

#[derive(Debug, clap::Args)]
struct ParseArgs {
    /// Path to the ecore file to parse
    #[arg(value_name = "FILE")]
    input: PathBuf,

    /// Output format
    #[arg(short, long, value_name = "FORMAT", default_value = "pretty")]
    output_format: OutputFormat,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Suppress output (only show errors)
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Pretty,
}

#[derive(Debug, clap::Args)]
struct GenerateArgs {
    /// Input Ecore metamodel path
    input: PathBuf,

    /// Output directory where the generated project is written
    #[arg(
        short = 'o',
        long = "output",
        default_value = ".output/generated_project"
    )]
    output_dir: PathBuf,

    /// Generated Cargo package name
    #[arg(short = 'p', long = "project-name")]
    project_name: Option<String>,

    /// Path to the Moirai workspace root
    #[arg(
        long = "moirai-root",
        default_value = "../moirai",
        env = "ATRAKTOS_MOIRAI_ROOT"
    )]
    moirai_root: PathBuf,

    /// Enable generator debug output
    #[arg(long = "debug")]
    debug: bool,

    /// Increase log verbosity (`-v`, `-vv`)
    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    verbose: u8,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Parse(args) => run_parse(args),
        Command::Generate(args) => run_generate(args),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", format!("Error: {err}").red());
            ExitCode::from(1)
        }
    }
}

fn run_parse(args: ParseArgs) -> Result<()> {
    if args.verbose {
        eprintln!("{}", "Verbose mode enabled".blue());
    }

    if !args.quiet {
        println!("{}", format!("Parsing: {}", args.input.display()).cyan());
    }

    let content = fs::read_to_string(&args.input)
        .map_err(|e| anyhow!("Failed to read file '{}': {}", args.input.display(), e))?;

    let ctx = Ctx::parse(&content).map_err(|e| anyhow!("Failed to parse ecore file: {}", e))?;

    if !args.quiet {
        println!("{}", "Parsing completed successfully ✓".green());
        println!();
    }

    match args.output_format {
        OutputFormat::Pretty => {
            for line in ctx.to_pretty_string().lines() {
                println!("| {}", line);
            }
        }
    }

    Ok(())
}

fn run_generate(args: GenerateArgs) -> Result<()> {
    init_logger(args.verbose);

    println!(
        "{} {}",
        "[INFO]".blue().bold(),
        "Starting code generation".bold()
    );

    let mut config = Config::new(args.input)
        .with_output_dir(args.output_dir)
        .with_moirai_root(args.moirai_root)
        .with_debug(args.debug);

    if let Some(project_name) = args.project_name {
        config = config.with_project_name(project_name);
    }

    let start = Instant::now();
    let report = generate_with_report(config)?;
    let elapsed = start.elapsed();

    println!(
        "{} {}",
        "[OK]".green().bold(),
        "Code generation completed".green().bold()
    );
    println!("{} {}", "input:".cyan().bold(), report.input_path.display());
    println!(
        "{} {}",
        "output:".cyan().bold(),
        report.output_dir.display()
    );
    println!("{} {}", "package:".cyan().bold(), report.package_name);
    println!("{} {}", "project:".cyan().bold(), report.project_name);
    println!(
        "{} {}",
        "classes:".cyan().bold(),
        report.class_count.to_string().yellow()
    );
    println!(
        "{} {}",
        "model.rs:".cyan().bold(),
        if report.model_generated {
            "generated".green().to_string()
        } else {
            "not generated".yellow().to_string()
        }
    );
    println!("{} {:.2?}", "duration:".cyan().bold(), elapsed);

    Ok(())
}

fn init_logger(verbosity: u8) {
    let default_level = match verbosity {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    let env = env_logger::Env::default().filter_or("RUST_LOG", default_level);
    env_logger::Builder::from_env(env).init();
}
