#[macro_export]
macro_rules! check_run_once {
    ($flag:expr, $msg:expr) => {
        if $flag.swap(true, std::sync::atomic::Ordering::Relaxed) {
            println!("{}", $msg);
            panic!();
        }
    };
}
