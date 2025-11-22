use std::panic;
use std::sync::{Arc, Once};

/// A struct that manages cleanup functions to be executed on process termination.
pub struct ExitHook {
    cleanup_fn: Arc<dyn Fn() + Send + Sync + 'static>,
}

impl ExitHook {
    /**
    Creates a new ExitHook with the given cleanup function.

    # Arguments

    * `f` - A function to be executed when the process exits

    # Example

    ```rust
    use cdp_html_shot::ExitHook;
    let hook = ExitHook::new(|| println!("Cleaning up..."));

    hook.register().unwrap();
    ```
    */
    pub fn new<F>(f: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        ExitHook {
            cleanup_fn: Arc::new(f),
        }
    }

    /// Registers all necessary hooks for process termination.
    pub fn register(&self) -> Result<(), Box<dyn std::error::Error>> {
        static INIT: Once = Once::new();
        let cleanup_fn = Arc::clone(&self.cleanup_fn);

        // Set up panic hook
        let original_hook = panic::take_hook();
        let panic_cleanup_fn = Arc::clone(&cleanup_fn);
        panic::set_hook(Box::new(move |panic_info| {
            panic_cleanup_fn();
            original_hook(panic_info);
        }));

        // Set up Ctrl+C handler
        INIT.call_once(|| {
            let ctrl_c_cleanup_fn = Arc::clone(&cleanup_fn);
            if let Err(e) = ctrlc::set_handler(move || {
                ctrl_c_cleanup_fn();
                std::process::exit(0);
            }) {
                eprintln!("Error setting Ctrl-C handler: {}", e);
            }
        });

        Ok(())
    }
}

impl Drop for ExitHook {
    fn drop(&mut self) {
        (self.cleanup_fn)();
    }
}
