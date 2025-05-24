#[macro_export]
macro_rules! check_run_once {
    ($msg:expr) => {
        static CHECK: AtomicBool = AtomicBool::new(false);
        if CHECK.swap(true, std::sync::atomic::Ordering::Relaxed) {
            println!("{}", $msg);
            panic!();
        }
    };
}
