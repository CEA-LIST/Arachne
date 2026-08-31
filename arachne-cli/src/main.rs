use std::{fs, path::PathBuf, process::ExitCode, time::Instant};

use anyhow::{Result, anyhow};
use arachne_codegen::{Config, MoiraiPathStyle, generate_with_report};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use colored::Colorize;
use ecore_rs::ctx::Ctx;
use log::{error, info};

#[derive(Debug, Parser)]
#[command(
    name = "arachne",
    version,
    about = "Arachne CLI: translates Ecore metamodels into collaborative, CRDT-based applications in Rust."
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
    /// Emit the JSON metamodel descriptor a node serves on `GET /api/metamodel`
    #[command(name = "describe")]
    Describe(DescribeArgs),
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

/// CLI mirror of [`MoiraiPathStyle`], so `clap` stays out of `arachne-codegen`.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum MoiraiPathStyleArg {
    /// Relative to the generated project — portable, keep the workspaces
    /// in the same relative arrangement
    Relative,
    /// Absolute — tied to the machine that ran the generator
    Absolute,
}

impl From<MoiraiPathStyleArg> for MoiraiPathStyle {
    fn from(value: MoiraiPathStyleArg) -> Self {
        match value {
            MoiraiPathStyleArg::Relative => Self::Relative,
            MoiraiPathStyleArg::Absolute => Self::Absolute,
        }
    }
}

#[derive(Debug, clap::Args)]
struct GenerateArgs {
    /// Input Ecore metamodel path
    input: PathBuf,

    /// Output directory where the generated project is written
    #[arg(short = 'o', long = "output")]
    output_dir: PathBuf,

    /// Generated Cargo package name
    #[arg(short = 'p', long = "project-name")]
    project_name: Option<String>,

    /// Path to the Moirai workspace root
    #[arg(
        short = 'm',
        long = "moirai-root",
        alias = "moirai-path",
        default_value = "../moirai",
        env = "ATRAKTOS_MOIRAI_ROOT"
    )]
    moirai_root: PathBuf,

    /// How Moirai `path` dependencies are written into the generated Cargo.toml
    #[arg(
        long = "moirai-path-style",
        value_name = "STYLE",
        default_value = "relative"
    )]
    moirai_path_style: MoiraiPathStyleArg,

    /// Increase log verbosity (`-v`, `-vv`)
    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    verbose: u8,
}

#[derive(Debug, clap::Args)]
struct DescribeArgs {
    /// Path to the ecore file to describe
    #[arg(value_name = "FILE")]
    input: PathBuf,

    /// Output file; stdout when omitted
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Parse(args) => run_parse(args),
        Command::Generate(args) => run_generate(args),
        Command::Describe(args) => run_describe(args),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{}", format!("Error: {err}").red());
            ExitCode::from(1)
        }
    }
}

fn run_parse(args: ParseArgs) -> Result<()> {
    if args.verbose {
        info!("{}", "Verbose mode enabled".blue());
    }

    if !args.quiet {
        info!("{}", format!("Parsing: {}", args.input.display()).cyan());
    }

    let content = fs::read_to_string(&args.input)
        .map_err(|e| anyhow!("Failed to read file '{}': {}", args.input.display(), e))?;

    let ctx = Ctx::parse(&content).map_err(|e| anyhow!("Failed to parse ecore file: {}", e))?;

    if !args.quiet {
        info!("{}", "Parsing completed successfully ✓".green());
    }

    match args.output_format {
        OutputFormat::Pretty => {
            for line in ctx.to_pretty_string().lines() {
                info!("| {}", line);
            }
        }
    }

    Ok(())
}

fn run_generate(args: GenerateArgs) -> Result<()> {
    init_logger(args.verbose);

    info!("{}", "Starting code generation".bold());

    let mut config = Config::new(args.input)
        .with_output_dir(args.output_dir)
        .with_moirai_root(args.moirai_root)
        .with_moirai_path_style(args.moirai_path_style.into());

    if let Some(project_name) = args.project_name {
        config = config.with_project_name(project_name);
    }

    let start = Instant::now();
    let report = generate_with_report(config)?;
    let elapsed = start.elapsed();

    info!(
        "{} {}",
        "[OK]".green().bold(),
        "Code generation completed".green().bold()
    );
    info!("{} {}", "input:".cyan().bold(), report.input_path.display());
    info!(
        "{} {}",
        "output:".cyan().bold(),
        report.output_dir.display()
    );
    info!("{} {}", "package:".cyan().bold(), report.package_name);
    info!("{} {}", "project:".cyan().bold(), report.project_name);
    info!(
        "{} {}",
        "classes:".cyan().bold(),
        report.class_count.to_string().yellow()
    );
    info!("{} {:.2?}", "duration:".cyan().bold(), elapsed);

    Ok(())
}

fn run_describe(args: DescribeArgs) -> Result<()> {
    let parser = arachne_codegen::EcoreParser::from_file(&args.input)
        .map_err(|e| anyhow!("Failed to parse '{}': {}", args.input.display(), e))?;
    let pack = arachne_codegen::find_user_package(&parser.ctx)?;
    let descriptor = arachne_codegen::descriptor_json(&parser.ctx, pack)?;
    let rendered = format!("{descriptor:#}\n");

    match args.output {
        Some(path) => fs::write(&path, rendered)
            .map_err(|e| anyhow!("Failed to write '{}': {}", path.display(), e))?,
        None => print!("{rendered}"),
    }

    Ok(())
}

fn init_logger(verbosity: u8) {
    let default_level = match verbosity {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    let env = env_logger::Env::default().filter_or("RUST_LOG", default_level);
    env_logger::Builder::from_env(env)
        .format_timestamp(None)
        .init();
}
