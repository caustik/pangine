use std::path::Path;
use std::process::Command;

#[test]
fn console_runs_the_route_cycle_program() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/route-cycle.pae");
    let output = Command::new(env!("CARGO_BIN_EXE_pangine-console")).arg(script).output().expect("run route-cycle example");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");

    assert!(output.status.success(), "console failed:\n{stderr}\n{stdout}");
    assert!(stderr.is_empty(), "unexpected stderr:\n{stderr}");
    assert!(stdout.contains("ps=   x3{[east]->[C]->[north]}x2{[north]->[B]->[east]}{[west]->[E]->[south]}"));

    let choice = stdout.find("ps> ['selected-route'] = ^['route-choice']").expect("Pangine choice command");
    let recorded = stdout.find("ps> $['recorded-outcome']").expect("recorded result read");

    assert!(choice < recorded);
    assert!(stdout[choice..].contains("ps=   {[east]->[C]->[north]}"));
    assert!(stdout[recorded..].contains("ps=   [success]"));
}
