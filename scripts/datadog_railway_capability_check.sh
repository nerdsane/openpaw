#!/usr/bin/env sh
set -eu

# Operational proof helper for ADR-0049. This does not enable Datadog products;
# it records whether the current Railway container has the host/kernel access
# those Datadog products require.

cap_eff_hex() {
  awk '/^CapEff:/ { print $2 }' /proc/self/status 2>/dev/null || true
}

has_cap() {
  name="$1"
  bit="$2"
  cap_eff="$(cap_eff_hex)"
  if [ -z "$cap_eff" ]; then
    return 1
  fi
  # POSIX shells used by Debian support hexadecimal arithmetic here. If the
  # platform shell cannot parse it, treat the capability as absent.
  if value=$((0x${cap_eff})) 2>/dev/null; then
    mask=$((1 << bit))
    [ $((value & mask)) -ne 0 ]
  else
    return 1
  fi
}

check_cap() {
  name="$1"
  bit="$2"
  if has_cap "$name" "$bit"; then
    printf 'true'
  else
    printf 'false'
  fi
}

check_path() {
  path="$1"
  if [ -e "$path" ]; then
    printf 'true'
  else
    printf 'false'
  fi
}

perf_event_paranoid="$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || printf 'unknown')"
ddprof_present=false
if command -v ddprof >/dev/null 2>&1; then
  ddprof_present=true
fi

cap_sys_admin="$(check_cap CAP_SYS_ADMIN 21)"
cap_sys_resource="$(check_cap CAP_SYS_RESOURCE 24)"
cap_sys_ptrace="$(check_cap CAP_SYS_PTRACE 19)"
cap_net_admin="$(check_cap CAP_NET_ADMIN 12)"
cap_net_raw="$(check_cap CAP_NET_RAW 13)"
cap_ipc_lock="$(check_cap CAP_IPC_LOCK 14)"
cap_chown="$(check_cap CAP_CHOWN 0)"
cap_perfmon="$(check_cap CAP_PERFMON 38)"

host_proc="$(check_path /host/proc)"
host_cgroup="$(check_path /host/sys/fs/cgroup)"
debugfs="$(check_path /sys/kernel/debug)"
lib_modules="$(check_path /lib/modules)"
system_probe_enabled="${DD_SYSTEM_PROBE_SERVICE_MONITORING_ENABLED:-false}"

usm_status="supported"
if [ "$system_probe_enabled" != "true" ] ||
  [ "$cap_sys_admin" != "true" ] ||
  [ "$cap_sys_resource" != "true" ] ||
  [ "$cap_sys_ptrace" != "true" ] ||
  [ "$cap_net_admin" != "true" ] ||
  [ "$cap_net_raw" != "true" ] ||
  [ "$cap_ipc_lock" != "true" ] ||
  [ "$cap_chown" != "true" ] ||
  [ "$host_proc" != "true" ] ||
  [ "$host_cgroup" != "true" ] ||
  [ "$debugfs" != "true" ] ||
  [ "$lib_modules" != "true" ]; then
  usm_status="blocked-on-Railway-system-probe"
fi

continuous_profiler_status="best-effort-canary-not-enabled"
if [ "${TEMPER_DDPROF_ENABLED:-false}" = "true" ]; then
  continuous_profiler_status="supported"
  if [ "$ddprof_present" != "true" ]; then
    continuous_profiler_status="blocked-on-Railway-perf-permissions"
  elif [ "$perf_event_paranoid" = "unknown" ]; then
    continuous_profiler_status="blocked-on-Railway-perf-permissions"
  elif [ "$perf_event_paranoid" -gt 2 ] 2>/dev/null && [ "$cap_perfmon" != "true" ]; then
    continuous_profiler_status="blocked-on-Railway-perf-permissions"
  fi
fi

cat <<EOF
{
  "usm_status": "$usm_status",
  "continuous_profiler_status": "$continuous_profiler_status",
  "system_probe": {
    "DD_SYSTEM_PROBE_SERVICE_MONITORING_ENABLED": "$system_probe_enabled",
    "CAP_SYS_ADMIN": $cap_sys_admin,
    "CAP_SYS_RESOURCE": $cap_sys_resource,
    "CAP_SYS_PTRACE": $cap_sys_ptrace,
    "CAP_NET_ADMIN": $cap_net_admin,
    "CAP_NET_RAW": $cap_net_raw,
    "CAP_IPC_LOCK": $cap_ipc_lock,
    "CAP_CHOWN": $cap_chown,
    "mounts": {
      "/host/proc": $host_proc,
      "/host/sys/fs/cgroup": $host_cgroup,
      "/sys/kernel/debug": $debugfs,
      "/lib/modules": $lib_modules
    }
  },
  "continuous_profiler": {
    "TEMPER_DDPROF_ENABLED": "${TEMPER_DDPROF_ENABLED:-false}",
    "ddprof_present": $ddprof_present,
    "perf_event_paranoid": "$perf_event_paranoid",
    "CAP_PERFMON": $cap_perfmon
  }
}
EOF

