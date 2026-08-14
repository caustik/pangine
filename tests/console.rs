use std::path::Path;
use std::process::Command;

#[test]
fn route_cycle_changes_its_complete_choice_after_repeated_outcomes() {
    let stdout = run_example("route-cycle.pae");

    assert_eq!(
        command_results(&stdout, "['route-choice'] /= $['negative-route']"),
        vec![
            "x3{[east]->[C]->[north]}x2{[north]->[B]->[east]}{[west]->[E]->[south]}",
            "x2{[east]->[C]->[north]}x2{[north]->[B]->[east]}{[west]->[E]->[south]}",
            "x2{[north]->[B]->[east]}{[east]->[C]->[north]}{[west]->[E]->[south]}",
            "x3{[north]->[B]->[east]}{[east]->[C]->[north]}{[west]->[E]->[south]}",
        ]
    );
    assert_eq!(
        command_results(&stdout, "['selected-route'] = ^['route-choice']"),
        vec!["{[east]->[C]->[north]}", "{[east]->[C]->[north]}", "{[north]->[B]->[east]}", "{[north]->[B]->[east]}"]
    );
    assert_eq!(command_results(&stdout, "$['recorded-outcome']"), vec!["[success]"]);
}

#[test]
fn setting_choice_collapses_three_linked_outputs_as_one_complete_result() {
    let stdout = run_example("settings-choice.pae");

    assert_eq!(command_results(&stdout, "$(['mode-choice']['amount-choice']['timing-choice'])"), vec!["x3([gentle][light][slow])x2([deep][fast][firm])"]);
    assert_eq!(
        command_results(&stdout, "['selected-settings'] = ^(([mode]->['mode-choice'])([amount]->['amount-choice'])([timing]->['timing-choice']))"),
        vec!["{[amount]->[light]}{[mode]->[gentle]}{[timing]->[slow]}"]
    );
    assert_eq!(command_results(&stdout, "$['selected-mode']"), vec!["[gentle]"]);
    assert_eq!(command_results(&stdout, "$['selected-amount']"), vec!["[light]"]);
    assert_eq!(command_results(&stdout, "$['selected-timing']"), vec!["[slow]"]);
    assert_eq!(command_results(&stdout, "$['recorded-outcome']"), vec!["[accepted]"]);
}

fn run_example(name: &str) -> String {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples").join(name);
    let output = Command::new(env!("CARGO_BIN_EXE_pangine-console")).arg(script).output().unwrap_or_else(|error| panic!("run {name}: {error}"));
    let status = output.status;
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");

    assert!(status.success(), "{name} failed:\n{stderr}\n{stdout}");
    assert!(stderr.is_empty(), "unexpected stderr from {name}:\n{stderr}");
    stdout
}

fn command_results<'a>(stdout: &'a str, command: &str) -> Vec<&'a str> {
    let prompt = format!("ps> {command}");
    let lines = stdout.lines().collect::<Vec<_>>();

    lines.windows(2).filter(|pair| pair[0] == prompt).map(|pair| pair[1].strip_prefix("ps=   ").expect("command result")).collect()
}
