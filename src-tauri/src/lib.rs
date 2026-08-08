mod github;
mod messenger;
mod ssh;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(ssh::session::SshState::new())
        // Keep the Messenger window warm: closing it just hides it (instant
        // reopen, notifications keep flowing). Use `messenger_close` to actually
        // reclaim its RAM.
        .on_window_event(|window, event| {
            if window.label() == messenger::MESSENGER_LABEL {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
