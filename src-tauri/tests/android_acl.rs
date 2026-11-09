use std::collections::BTreeMap;
use tauri::utils::{
    acl::{capability::Capability, manifest::Manifest, resolved::Resolved, ExecutionContext},
    platform::Target,
};

fn resolve_android_capability(target: Target) -> Resolved {
    let capability: Capability =
        serde_json::from_str(include_str!("../capabilities/android.json")).unwrap();
    // tauri-build generates these from the actual dependency permission files.
    let manifests: BTreeMap<String, Manifest> =
        serde_json::from_str(include_str!("../gen/schemas/acl-manifests.json")).unwrap();
    Resolved::resolve(
        &manifests,
        BTreeMap::from([(capability.identifier.clone(), capability)]),
        target,
    )
    .unwrap()
}

#[test]
fn android_main_window_can_open_and_create_documents() {
    let acl = resolve_android_capability(Target::Android);
    for command in ["plugin:saf|open_document", "plugin:saf|create_document"] {
        let grants = acl.allowed_commands.get(command).unwrap();
        assert!(
            grants.iter().any(|grant| {
                grant.context == ExecutionContext::Local
                    && grant.windows.iter().any(|pattern| pattern.matches("main"))
            }),
            "{command} must be callable from the main app window"
        );
        assert!(
            grants.iter().all(|grant| {
                grant.webviews.is_empty()
                    && !grant
                        .windows
                        .iter()
                        .any(|pattern| pattern.matches("settings"))
                    && grant.context == ExecutionContext::Local
            }),
            "{command} must not grant unrelated windows or remote pages access"
        );
        assert!(!acl.denied_commands.contains_key(command));
    }
}

#[test]
fn saf_picker_permissions_remain_android_only() {
    for target in [Target::Windows, Target::Linux, Target::MacOS, Target::Ios] {
        assert!(resolve_android_capability(target)
            .allowed_commands
            .is_empty());
    }
}
