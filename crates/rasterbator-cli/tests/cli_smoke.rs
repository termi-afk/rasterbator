//! End-to-end smoke tests: run the built binary against the shipped test
//! images and check that a valid PDF appears.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rasterbator")
}

fn test_image(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test-images")
        .join(name)
}

fn temp_pdf(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rasterbator-{tag}-{}.pdf",
        std::process::id()
    ))
}

#[test]
fn modern_smoke_renders_one_page_pdf() {
    let out = temp_pdf("modern-smoke");
    let _ = fs::remove_file(&out);

    let output = Command::new(bin())
        .arg(test_image("tiny.png"))
        .args([
            "--mode", "modern",
            "--size", "120x120",
            "--dot-size", "20",
            "--no-atomic",
            "--print-summary",
        ])
        .arg("-o")
        .arg(&out)
        .output()
        .expect("failed to run rasterbator");

    assert!(
        output.status.success(),
        "modern run failed: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let bytes = fs::read(&out).expect("no PDF produced");
    assert!(bytes.starts_with(b"%PDF"), "output is not a PDF");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"pages\":1"),
        "expected a one-page summary, got: {stdout}"
    );

    let _ = fs::remove_file(&out);
}

#[test]
fn legacy_smoke_renders_pdf() {
    let out = temp_pdf("legacy-smoke");
    let _ = fs::remove_file(&out);

    let output = Command::new(bin())
        .arg(test_image("tiny.png"))
        .args(["--size", "120x120", "--dot-size", "20"])
        .arg("-o")
        .arg(&out)
        .output()
        .expect("failed to run rasterbator");

    assert!(
        output.status.success(),
        "legacy run failed: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let bytes = fs::read(&out).expect("no PDF produced");
    assert!(bytes.starts_with(b"%PDF"), "output is not a PDF");

    let _ = fs::remove_file(&out);
}
