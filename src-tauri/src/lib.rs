mod awsauth;
mod devkon;
mod github;
mod gitmod;
mod memtrack;
mod messenger;
mod security;
mod ssh;
mod utils;
mod wpdeploy;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri_plugin_global_shortcut::ShortcutState;

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Persist the main window's size/position/maximized state across restarts.
        // Denylist the Messenger + peek windows: the bubble manages its own
        // geometry (see `messenger::bubble`), so we don't let the plugin fight it.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&[messenger::MESSENGER_LABEL, messenger::PEEK_LABEL])
                .build(),
        )
        // Global shortcut that toggles the Messenger window between full and
        // bubble. The combo is user-configurable (Messenger page) and registered
        // in `setup`; this only routes the keypress to the toggle handler.
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        messenger::bubble::on_shortcut(app);
                    }
                })
                .build(),
        )
        .manage(ssh::session::SshState::new())
        .manage(messenger::bubble::BubbleState::new())
        // Open the RAM-history store and start the 15-min background sampler. A
        // failure here (corrupt/locked/unwritable DB) must never abort startup and
        // take down SSH/GitHub/Messenger, so it degrades to a disabled store and
        // the /memory page reports no data instead.
        .setup(|app| {
            match memtrack::init(app.handle()) {
                Ok(store) => {
                    app.manage(store);
                    memtrack::spawn_sampler(app.handle().clone());
                }
                Err(e) => {
                    eprintln!("memtrack: disabled, init failed: {e}");
                    app.manage(memtrack::MemStore::disabled());
                }
            }
            // Register the persisted (or default) Messenger toggle shortcut.
            messenger::bubble::init_shortcut(app.handle());
            Ok(())
        })
        // Messenger window events: keep it warm on close (hide, not destroy) and
        // snap the floating bubble to a screen edge after a drag. See
        // `messenger::bubble::on_window_event`.
        .on_window_event(|window, event| {
            if window.label() == messenger::MESSENGER_LABEL {
                messenger::bubble::on_window_event(window, event);
            }
        })
        .invoke_handler(tauri::generate_handler![
            github::commands::fetch_pull_requests,
            github::commands::readd_ci_label,
            github::commands::fetch_ci_label_counts,
            github::commands::fetch_unresolved_comment_counts,
            github::commands::fetch_merge_queue_status,
            github::commands::fetch_pr_checks,
            github::views::pr_views_list,
            github::views::pr_views_save,
            github::views::pr_views_delete,
            github::views::pr_views_set_active,
            ssh::commands::hosts_list,
            ssh::commands::host_save,
            ssh::commands::host_delete,
            ssh::commands::discover_history_hosts,
            ssh::commands::ssh_build_command,
            ssh::commands::ssh_set_passphrase,
            ssh::commands::ssh_connect,
            ssh::commands::ssh_trust_hostkey,
            ssh::commands::ssh_write,
            ssh::commands::ssh_resize,
            ssh::commands::ssh_disconnect,
            ssh::commands::forward_start,
            ssh::commands::forward_stop,
            ssh::commands::forwards_list,
            messenger::commands::messenger_open,
            messenger::commands::messenger_close,
            messenger::commands::messenger_get_shortcut,
            messenger::commands::messenger_set_shortcut,
            messenger::commands::messenger_get_idle_secs,
            messenger::commands::messenger_set_idle_secs,
            messenger::commands::messenger_get_muted,
            messenger::commands::messenger_set_muted,
            messenger::commands::messenger_get_link_rules,
            messenger::commands::messenger_set_link_rules,
            memtrack::commands::memory_history,
            memtrack::commands::memory_latest,
            memtrack::commands::memory_snapshot_at,
            memtrack::commands::memory_snapshot_now,
            utils::commands::cisco_status,
            utils::commands::cisco_set_enabled,
            utils::commands::cisco_get_config,
            utils::commands::cisco_set_config,
            utils::commands::pick_directory,
            devkon::commands::devkon_list,
            devkon::commands::devkon_save,
            devkon::commands::devkon_delete,
            devkon::commands::devkon_branches,
            devkon::commands::devkon_deploy,
            devkon::commands::devkon_destroy,
            devkon::commands::devkon_status,
            devkon::commands::devkon_set_config,
            awsauth::commands::awsauth_get_config,
            awsauth::commands::awsauth_set_config,
            awsauth::commands::awsauth_open_brave,
            awsauth::commands::awsauth_check_fresh,
            awsauth::commands::awsauth_finish,
            gitmod::commands::gitmod_get_config,
            gitmod::commands::gitmod_set_config,
            gitmod::commands::gitmod_status,
            gitmod::commands::gitmod_switch,
            gitmod::commands::gitmod_refresh_pull,
            gitmod::commands::gitmod_switch_all,
            gitmod::commands::gitmod_open_app,
            wpdeploy::commands::wpdeploy_config_get,
            wpdeploy::commands::wpdeploy_config_save,
            wpdeploy::commands::wpdeploy_config_reset,
            wpdeploy::commands::wpdeploy_set_docroot,
            wpdeploy::commands::wpdeploy_products,
            wpdeploy::commands::wpdeploy_detect_docroot,
            wpdeploy::commands::wpdeploy_deploy,
            wpdeploy::commands::wpdeploy_rollback,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
