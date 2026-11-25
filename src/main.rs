#[cfg(target_os = "linux")]
mod linux_daemon;

#[cfg(target_os = "linux")]
fn main() -> touchpadflow::error::Result<()> {
    linux_daemon::main()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!(
        "The touchpadflow daemon is Linux-only. Use touchpadflow-macos (macos/) or \
         TouchpadFlow.exe (windows/) as the engine, and touchpadflow-app for settings."
    );
    std::process::exit(1);
}
