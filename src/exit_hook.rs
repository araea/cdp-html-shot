use std::sync::{Arc, Once};

pub struct ExitHook {
    func: Arc<dyn Fn() + Send + Sync>,
}

impl ExitHook {
    pub fn new<F: Fn() + Send + Sync + 'static>(f: F) -> Self {
        Self { func: Arc::new(f) }
    }

    pub fn register(&self) -> Result<(), Box<dyn std::error::Error>> {
        static ONCE: Once = Once::new();
        let f = self.func.clone();
        let res = Ok(());
        ONCE.call_once(|| {
            if let Err(e) = ctrlc::set_handler(move || {
                f();
                std::process::exit(0);
            }) {
                eprintln!("Ctrl+C handler error: {}", e);
            }
        });
        res
    }
}

impl Drop for ExitHook {
    fn drop(&mut self) {
        (self.func)();
    }
}
