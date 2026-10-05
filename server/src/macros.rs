#[macro_export]
macro_rules! check_run_once {
    ($msg:expr) => {
        static CHECK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if CHECK.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!("{}", $msg);
            panic!();
        }
    };
}
