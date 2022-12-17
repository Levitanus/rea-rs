use reaper_test::{run_integration_test, ReaperVersion};
#[test]
fn main() {
    run_integration_test(ReaperVersion::latest());
}
