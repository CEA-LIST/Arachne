use std::io::{BufWriter, Write};
use std::{fs, path::PathBuf, process::ExitCode, time::Instant};

use anyhow::{Context, Result, anyhow};
use arachne_codegen::{Config, generate_with_report};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use colored::Colorize;
use ecore_rs::ctx::Ctx;
use log::{error, info};

mod import_sirius;

#[derive(Debug, Parser)]
#[command(
    name = "arachne",
    version,
    about = "Arachne CLI: translates Ecore metamodels into collaborative, CRDT-based applications in Rust."
)]
struct Cli {
    /// Emit newline-delimited JSON diagnostics and a final result on stdout
    #[arg(long, global = true)]
    diagnostics_json: bool,
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

    /// Print the identity of a metamodel descriptor file: its `nsURI` and
    /// the digest a node keys it by
    #[command(name = "digest")]
    Digest(DigestArgs),

    /// Import a Sirius Web / SysON project into a live model, or print its offline batch
    #[command(name = "import-sirius")]
    ImportSirius(ImportSiriusArgs),
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
    #[arg(short = 'o', long = "output")]
    output_dir: PathBuf,

    /// Generated Cargo package name
    #[arg(short = 'p', long = "project-name")]
    project_name: Option<String>,

    /// Use crates from a local Moirai workspace
    #[arg(long, value_name = "DIRECTORY")]
    moirai_path: Option<PathBuf>,

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

#[derive(Debug, clap::Args)]
struct DigestArgs {
    /// Path to a metamodel descriptor (`.metamodel.json`)
    #[arg(value_name = "FILE")]
    input: PathBuf,
}

#[derive(Debug, clap::Args)]
struct ImportSiriusArgs {
    /// Path to the project export (`{"json", "ns", "content"}`)
    #[arg(value_name = "PROJECT")]
    input: PathBuf,

    /// Path to the metamodel descriptor the model is registered under
    #[arg(long, value_name = "DESCRIPTOR")]
    descriptor: PathBuf,

    /// Node hosting the already registered model
    #[arg(long, value_name = "URL", requires = "model")]
    node: Option<String>,

    /// Registered model to import into (requires --node)
    #[arg(long, value_name = "ID", requires = "node")]
    model: Option<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.diagnostics_json;
    let stage = match &cli.command {
        Command::Parse(_) => "parse",
        Command::Generate(_) => "generate",
        Command::Describe(_) => "describe",
        Command::Digest(_) => "digest",
        Command::ImportSirius(_) => "import-sirius",
    };
    if json {
        log::set_logger(&JSON_LOGGER).expect("logger initialized once");
        log::set_max_level(log::LevelFilter::Info);
    }

    let result = match cli.command {
        Command::Parse(args) => run_parse(args, json),
        Command::Generate(args) => run_generate(args, json),
        Command::Describe(args) => run_describe(args, json),
        Command::Digest(args) => run_digest(args, json),
        Command::ImportSirius(args) => run_import_sirius(args, json),
    };

    if json {
        let diagnostic = result.as_ref().err().map(|err| {
            let (code, kind) = error_category(err, stage);
            serde_json::json!({"code": code, "kind": kind, "message": format!("{err:#}")})
        });
        println!(
            "{}",
            serde_json::json!({
                "reason": "arachne-result", "schema_version": 1,
                "stage": stage, "ok": result.is_ok(), "error": diagnostic,
            })
        );
        return if result.is_ok() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{}", format!("Error: {err}").red());
            ExitCode::from(1)
        }
    }
}

fn run_parse(args: ParseArgs, json: bool) -> Result<()> {
    if !json {
        if args.quiet {
            init_logger_at("error");
        } else {
            init_logger(u8::from(args.verbose));
        }
    }

    if args.verbose {
        info!("{}", "Verbose mode enabled".blue());
    }

    if !args.quiet {
        info!("{}", format!("Parsing: {}", args.input.display()).cyan());
    }

    let content = fs::read_to_string(&args.input)
        .with_context(|| format!("Failed to read file '{}'", args.input.display()))?;

    let ctx = Ctx::parse(&content).map_err(|e| anyhow!("Failed to parse ecore file: {}", e))?;

    if !args.quiet {
        info!("{}", "Parsing completed successfully ✓".green());
    }

    if !args.quiet && !json {
        match args.output_format {
            OutputFormat::Pretty => {
                for line in ctx.to_pretty_string().lines() {
                    info!("| {}", line);
                }
            }
        }
    }

    Ok(())
}

fn run_generate(args: GenerateArgs, json: bool) -> Result<()> {
    if !json {
        init_logger(args.verbose);
    }

    info!("{}", "Starting code generation".bold());

    let mut config = Config::new(args.input).with_output_dir(args.output_dir);

    if let Some(project_name) = args.project_name {
        config = config.with_project_name(project_name);
    }

    if let Some(path) = args.moirai_path {
        config = config.with_moirai_path(path);
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

fn run_describe(args: DescribeArgs, json: bool) -> Result<()> {
    // `main` reports a failure through `log::error!`, which writes nothing until a logger is
    // installed, so a refusal from here — a metamodel using Ecore's own classes, say — used to
    // exit 1 in silence. `error` keeps stdout a descriptor and says why when there is none.
    if !json {
        init_logger_at("error");
    }
    // The parser's error goes out unchanged: it is the line Léo's `generate` prints for the same file.
    // An unreadable file names its path as in f90795e; told apart by variant, never by message.
    let described = arachne_codegen::describe(&args.input).map_err(|e| match e {
        e @ arachne_codegen::ArachneError::FileRead(_) => {
            anyhow!("Failed to parse '{}': {e}", args.input.display())
        }
        e => anyhow::Error::from(e),
    })?;
    let descriptor = &described.descriptor;
    let rendered = format!("{descriptor:#}\n");

    match args.output {
        Some(path) => {
            fs::write(&path, rendered)
                .map_err(|e| anyhow!("Failed to write '{}': {}", path.display(), e))?;
            described.write_dependencies(path.parent().unwrap_or(".".as_ref()))?;
        }
        None => print!("{rendered}"),
    }
    // On stderr, so the descriptor on stdout stays a descriptor.
    eprintln!("{}", metamodel_id(descriptor));

    Ok(())
}

/// `{nsURI, digest}` for a descriptor file, as a registration names it.
fn run_digest(args: DigestArgs, json: bool) -> Result<()> {
    if !json {
        init_logger_at("error");
    }
    let text = fs::read_to_string(&args.input)
        .map_err(|e| anyhow!("Failed to read '{}': {}", args.input.display(), e))?;
    let descriptor: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| anyhow!("'{}' is not a JSON descriptor: {}", args.input.display(), e))?;
    println!("{}", metamodel_id(&descriptor));
    Ok(())
}

/// Live import when a node and model are supplied; otherwise the legacy offline batch.
fn run_import_sirius(args: ImportSiriusArgs, json: bool) -> Result<()> {
    if !json {
        init_logger_at("error");
    }
    let read = |path: &PathBuf, what: &str| -> Result<serde_json::Value> {
        let text = fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read '{}': {}", path.display(), e))?;
        serde_json::from_str(&text)
            .map_err(|e| anyhow!("'{}' is not a JSON {what}: {}", path.display(), e))
    };
    let descriptor = read(&args.descriptor, "descriptor")?;
    let project = read(&args.input, "project export")?;
    if let (Some(node), Some(model)) = (&args.node, &args.model) {
        return import_sirius::live::import(&descriptor, &project, node, model)
            .map_err(|err| anyhow!("{err:#}"));
    }
    let import = import_sirius::import(&descriptor, &project).map_err(|failures| {
        anyhow!(
            "'{}' cannot be carried by the descriptor:\n  {}",
            args.input.display(),
            failures.join("\n  ")
        )
    })?;

    let mut out = BufWriter::new(std::io::stdout().lock());
    out.write_all(b"{\"ops\":")?;
    serde_json::to_writer(&mut out, &import.ops)?;
    out.write_all(b"}\n")?;
    out.flush()?;

    eprintln!("metamodel: {}", metamodel_id(&descriptor));
    eprintln!("operations: {}", import.ops.len());
    for missing in &import.unresolved {
        eprintln!(
            "unresolved: {} {} {}",
            missing.holder, missing.feature, missing.value
        );
    }
    eprintln!(
        "references: {} resolved, {} unresolved",
        import.resolved,
        import.unresolved.len()
    );
    Ok(())
}

/// The identity of a parsed descriptor: its `nsURI` beside the digest a node
/// keys it by.
fn metamodel_id(descriptor: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "nsURI": descriptor.get("nsURI").cloned().unwrap_or(serde_json::Value::Null),
        "digest": arachne_codegen::metamodel_digest(descriptor),
    })
}

fn init_logger(verbosity: u8) {
    let default_level = match verbosity {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    init_logger_at(default_level);
}

fn init_logger_at(default_level: &str) {
    let env = env_logger::Env::default().filter_or("RUST_LOG", default_level);
    env_logger::Builder::from_env(env)
        .format_timestamp(None)
        .init();
}

struct JsonLogger;
static JSON_LOGGER: JsonLogger = JsonLogger;

impl log::Log for JsonLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            println!(
                "{}",
                serde_json::json!({
                    "reason": "arachne-diagnostic", "level": match record.level() {
                        log::Level::Warn => "warning",
                        log::Level::Error => "error",
                        log::Level::Info => "info",
                        log::Level::Debug => "debug",
                        log::Level::Trace => "trace",
                    },
                    "code": record.target(), "message": record.args().to_string(),
                })
            );
        }
    }

    fn flush(&self) {}
}

fn error_category(err: &anyhow::Error, stage: &str) -> (&'static str, &'static str) {
    use arachne_codegen::ArachneError;
    if err.downcast_ref::<std::io::Error>().is_some() {
        return ("io", "infrastructure");
    }
    if let Some(err) = err.downcast_ref::<ArachneError>() {
        return match err {
            ArachneError::FileRead(_) | ArachneError::Config(_) => {
                ("configuration", "infrastructure")
            }
            ArachneError::RootClassNotFound(_) => ("root_selection", "unsupported_feature"),
            ArachneError::NoValidPackageFound => ("package_selection", "unsupported_feature"),
            ArachneError::DuplicateDescriptorClass(_)
            | ArachneError::DescriptorNotClosed { .. } => ("invalid_model", "invalid_input"),
            ArachneError::InvalidModel(_) => ("invalid_model", "invalid_input"),
            ArachneError::SynParse(_) => ("generated_syntax", "implementation_bug"),
            ArachneError::EcoreParse(_) => ("parse_error", "unknown"),
            ArachneError::CodeGeneration(_) => ("generation_error", "unknown"),
        };
    }
    if stage == "parse" {
        ("parse_error", "unknown")
    } else {
        ("generation_error", "unknown")
    }
}
