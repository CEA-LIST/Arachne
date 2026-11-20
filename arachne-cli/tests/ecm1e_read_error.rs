//! EC.M1e: `describe` keeps f90795e's line when the file cannot be read, and passes a refusal
//! from the parser through as Léo's ebc3141 build prints it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn first_stderr_line(input: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_arachne"))
        .arg("describe")
        .arg(input)
        .output()
        .expect("run arachne describe");
    assert!(!out.status.success(), "describe should exit non-zero on {}", input.display());
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut line = String::new();
    let mut chars = stderr.lines().next().unwrap_or("").chars();
    // Remove ANSI color sequences (ESC [ ... m).
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            chars.by_ref().find(|&c| c == 'm');
        } else {
            line.push(c);
        }
    }
    line
}

/// f90795e's form, with the io error the OS gives for this path.
fn read_error_line(input: &Path) -> String {
    let io = std::fs::read_to_string(input).expect_err("input must not be readable");
    format!(
        "[ERROR arachne] Error: Failed to parse '{}': Failed to read Ecore file: {io}",
        input.display()
    )
}

#[test]
fn missing_file_names_the_path() {
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ecm1e-missing.ecore");
    let _ = std::fs::remove_file(&missing);
    assert_eq!(first_stderr_line(&missing), read_error_line(&missing));
}

#[test]
fn directory_names_the_path() {
    // A path that exists but cannot be read as a file: the line still comes from the read error.
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    assert_eq!(first_stderr_line(&dir), read_error_line(&dir));
}

#[test]
fn parser_refusal_passes_through() {
    // his_line for up__arachne-parser__rsc__aas.ecore in EC.M1 a2/o4 his.csv (Léo's ebc3141).
    let aas = Path::new(env!("CARGO_MANIFEST_DIR")).join("../arachne-parser/rsc/aas.ecore");
    assert_eq!(
        first_stderr_line(&aas),
        "[ERROR arachne] Error: Failed to parse Ecore metamodel: unsupported `eType` path `ecore:EClass ../../org.eclipse.uml2.uml/model/UML.ecore#//Package`"
    );
}
