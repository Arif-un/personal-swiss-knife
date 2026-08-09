#!/usr/bin/env bash
#
# cisco-umbrella.sh - enable/disable Cisco Umbrella on macOS.
#
# Cisco Umbrella runs as the `acumbrellaagent` process, which is spawned and
# kept alive by the `vpnagentd` launchd daemon (label:
# com.cisco.secureclient.vpn.service.agent, KeepAlive=true). Because the agent
# has no launchd job of its own, `kill acumbrellaagent` just respawns. There are
# two real control points:
#
#   umbrella  (default) - move the Umbrella profile (OrgInfo.json) aside and
#                         restart the daemon. Only Umbrella goes dark; the VPN
#                         side of Cisco Secure Client keeps working.
#   all                 - bootout/bootstrap the whole Secure Client daemon.
#                         Stops VPN and Umbrella together.
#
# Requires root (auto re-execs via sudo). On a Jamf/MDM-managed Mac the profile
# or daemon may be re-pushed/re-enabled by policy.
#
# Usage:
#   sudo ./cisco-umbrella.sh status
#   sudo ./cisco-umbrella.sh disable [umbrella|all]
#   sudo ./cisco-umbrella.sh enable  [umbrella|all]

set -euo pipefail

LABEL="com.cisco.secureclient.vpn.service.agent"
PLIST="/opt/cisco/secureclient/bin/Cisco Secure Client - AnyConnect VPN Service.app/Contents/Library/LaunchDaemons/com.cisco.secureclient.vpn.service.agent.plist"
ORGINFO="/opt/cisco/secureclient/umbrella/OrgInfo.json"
ORGINFO_OFF="${ORGINFO}.disabled"

require_root() {
  if [ "$(id -u)" -ne 0 ]; then
    echo "Re-running with sudo..."
    exec sudo "$0" "$@"
  fi
}

restart_daemon() {
  # bootout is idempotent; ignore "not loaded". bootstrap re-registers it.
  launchctl bootout "system/${LABEL}" 2>/dev/null || true
  sleep 1
  launchctl bootstrap system "$PLIST" 2>/dev/null || true
}

status() {
  local pids
  pids="$(pgrep -x acumbrellaagent || true)"
  if [ -n "$pids" ]; then
    echo "Umbrella agent : RUNNING (pid ${pids//$'\n'/ })"
  else
    echo "Umbrella agent : stopped"
  fi
  if pgrep -q -x vpnagentd; then
    echo "Secure Client  : RUNNING (vpnagentd)"
  else
    echo "Secure Client  : stopped"
  fi
  if [ -f "$ORGINFO" ]; then
    echo "Umbrella profile: present (enabled)"
  elif [ -f "$ORGINFO_OFF" ]; then
    echo "Umbrella profile: moved aside (disabled by this script)"
  else
    echo "Umbrella profile: absent"
  fi
}

action="${1:-status}"
scope="${2:-umbrella}"

case "$action" in
  status)
    status
    ;;

  disable|off|stop)
    require_root "$@"
    case "$scope" in
      umbrella)
        if [ -f "$ORGINFO" ]; then
          mv "$ORGINFO" "$ORGINFO_OFF"
          echo "Moved Umbrella profile aside: $ORGINFO_OFF"
        else
          echo "Umbrella profile already absent."
        fi
        restart_daemon
        echo "Cisco Umbrella disabled (VPN left intact)."
        ;;
      all)
        launchctl bootout "system/${LABEL}" 2>/dev/null || true
        launchctl disable "system/${LABEL}" 2>/dev/null || true
        echo "Cisco Secure Client daemon stopped and disabled (VPN + Umbrella)."
        ;;
      *) echo "unknown scope: $scope (use: umbrella|all)"; exit 1 ;;
    esac
    echo; status
    ;;

  enable|on|start)
    require_root "$@"
    case "$scope" in
      umbrella)
        if [ -f "$ORGINFO_OFF" ]; then
          mv "$ORGINFO_OFF" "$ORGINFO"
          echo "Restored Umbrella profile: $ORGINFO"
        else
          echo "No saved profile to restore (already enabled or never disabled here)."
        fi
        restart_daemon
        echo "Cisco Umbrella enabled."
        ;;
      all)
        launchctl enable "system/${LABEL}" 2>/dev/null || true
        launchctl bootstrap system "$PLIST" 2>/dev/null || true
        echo "Cisco Secure Client daemon enabled and started (VPN + Umbrella)."
        ;;
      *) echo "unknown scope: $scope (use: umbrella|all)"; exit 1 ;;
    esac
    echo; status
    ;;

  *)
    echo "usage: $0 {status|enable|disable} [umbrella|all]"
    exit 1
    ;;
esac
