/// Panics with `$msg` if the code around it runs a second time, for loops that must run
/// only once per process.
macro_rules! check_run_once {
    ($msg:expr) => {
        static CHECK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if CHECK.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!("{}", $msg);
            panic!();
        }
    };
}

pub(crate) use check_run_once;
