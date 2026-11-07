//! Installed only as a root-owned DEB helper. Never run the desktop app as root.
#[path = "../update_lifecycle.rs"]
#[allow(dead_code)]
mod update_lifecycle;
fn main() {
    if std::env::args_os().len() != 1 {
        eprintln!("This installer accepts a signed update on stdin, not arguments.");
        std::process::exit(1);
    }
    let _guard = match update_lifecycle::system_guard(true) {
        Ok(guard) => guard,
        Err(error) => {
            eprintln!("Matrix Play update rejected: {error}");
            std::process::exit(1);
        }
    };
    #[cfg(unix)]
    let result = matrix_update_core::deb::install_from_stdin(
        &mut std::io::stdin().lock(),
        env!("CARGO_PKG_VERSION"),
    );
    #[cfg(not(unix))]
    let result: Result<(), String> = Err("The system updater is Linux-only".into());
    if let Err(error) = result {
        eprintln!("Matrix Play update rejected: {error}");
        std::process::exit(1);
    }
}
