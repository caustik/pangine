use pangine::Pangine;

fn main() {
    let mut pangine = Pangine::new();

    let stored = pangine.reference_concept("['memory'] = [cat]->[purrs]").expect("valid Pangine expression").expect("stored concept");
    let recalled = pangine.reference_concept("$['memory']").expect("valid Pangine expression").expect("recalled concept");

    let stored = pangine.format_concept(&stored, false);
    let recalled = pangine.format_concept(&recalled, false);

    assert_eq!(stored, recalled);
    println!("stored:   {stored}");
    println!("recalled: {recalled}");
}
