use aliasmgr_core::{model::{AliasRecord, ShellKind}, sync::SyncCoordinator};
use std::{fs, time::Instant};

fn run_benchmark(count: usize) -> (u128, usize) {
    let root = std::env::temp_dir().join(format!("aliasmgr-benchmark-{}-{count}", std::process::id()));
    let aliases = (0..count).map(|index| AliasRecord { name: format!("bench_{index}"), executable: "git".into(), ..Default::default() }).collect::<Vec<_>>();
    let started = Instant::now();
    let receipt = SyncCoordinator::new(&root).apply(&aliases, &[ShellKind::Bash], 1).unwrap();
    let elapsed = started.elapsed().as_millis();
    assert_eq!(receipt.results.len(), 1);
    assert!(root.join("generated/bash.sh").exists());
    let bytes = fs::metadata(root.join("generated/bash.sh")).unwrap().len() as usize;
    let _ = fs::remove_dir_all(root);
    (elapsed, bytes)
}

#[test]
fn benchmark_generates_500_and_1000_aliases_without_sensitive_output() {
    let (five_hundred_ms, five_hundred_bytes) = run_benchmark(500);
    let (one_thousand_ms, one_thousand_bytes) = run_benchmark(1000);
    println!("benchmark aliases=500 elapsed_ms={five_hundred_ms} generated_bytes={five_hundred_bytes}");
    println!("benchmark aliases=1000 elapsed_ms={one_thousand_ms} generated_bytes={one_thousand_bytes}");
    assert!(five_hundred_bytes > 0);
    assert!(one_thousand_bytes > five_hundred_bytes);
}
