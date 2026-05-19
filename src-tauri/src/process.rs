pub fn command(program: &str) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    hide_window(&mut command);
    command
}

#[cfg(target_os = "windows")]
fn hide_window(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_window(_command: &mut std::process::Command) {}
