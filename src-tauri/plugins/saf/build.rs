const COMMANDS: &[&str] = &["open_document", "create_document"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
