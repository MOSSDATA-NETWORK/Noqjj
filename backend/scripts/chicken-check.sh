#!/bin/bash
# chicken-check.sh — PVE 切鸡检测脚本
# 部署到 PVE 宿主机后自动执行
# 用法: chicken-check.sh [--vmid <id>] [--all] [--check-agent]
# 输出: JSON 格式结果

set -uo pipefail

# 重定向 stderr 到 /dev/null，防止干扰 JSON 输出
exec 2>/dev/null

# 全局超时（4分钟）
GLOBAL_TIMEOUT=240
( sleep $GLOBAL_TIMEOUT; kill -9 $$ 2>/dev/null ) &
WATCHDOG_PID=$!
trap "kill $WATCHDOG_PID 2>/dev/null" EXIT

MODE="all"
TARGET_VMID=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --vmid) TARGET_VMID="$2"; MODE="single"; shift 2 ;;
        --all) MODE="all"; shift ;;
        --check-agent) echo '{"ok":true,"agent":"installed"}'; kill $WATCHDOG_PID 2>/dev/null; exit 0 ;;
        *) shift ;;
    esac
done

# 检查特征（文件/服务/history/网络）
check_evidence() {
    local root="$1"
    local found=""
    [ -d "$root/opt/incus" ] && found="${found}incus_dir "
    [ -f "$root/usr/local/bin/incushlii-agent" ] && found="${found}incushlii_agent "
    [ -f "$root/usr/local/bin/nodeget-agent" ] && found="${found}nodeget_agent "
    [ -d "$root/var/lib/incus" ] && found="${found}incus_data "
    [ -d "$root/var/lib/lxd" ] && found="${found}lxd "

    if [ "$root" = "/" ]; then
        # GA 模式：可以检查 systemd 和网络
        local svc_count
        svc_count=$(ls /etc/systemd/system/ 2>/dev/null | grep -ciE "incus|shlii|nodeget" || true)
        [ "$svc_count" -gt 0 ] 2>/dev/null && found="${found}systemd_svc:${svc_count} "

        local hist
        hist=$(grep -ciE "shlii\.io|incushlii|nodeget|nodehatch" /root/.bash_history 2>/dev/null || true)
        [ "$hist" -gt 0 ] 2>/dev/null && found="${found}history:${hist} "

        local net
        net=$(ss -tnp 2>/dev/null | grep -ciE "nodeget|incushlii|ji\.778822|nodehatch" || true)
        [ "$net" -gt 0 ] 2>/dev/null && found="${found}network:${net} "
    else
        # 磁盘挂载模式
        local svc_count
        svc_count=$(ls "$root/etc/systemd/system/" 2>/dev/null | grep -ciE "incus|shlii|nodeget" || true)
        [ "$svc_count" -gt 0 ] 2>/dev/null && found="${found}systemd_svc:${svc_count} "

        local hist
        hist=$(grep -ciE "shlii\.io|incushlii|nodeget|nodehatch" "$root/root/.bash_history" 2>/dev/null || true)
        [ "$hist" -gt 0 ] 2>/dev/null && found="${found}history:${hist} "
    fi

    echo "$found"
}

# GA 方式检测
detect_ga() {
    local vmid="$1"
    local found
    found=$(check_evidence "/")
    if [ -n "$found" ]; then
        echo "{\"vmid\":\"$vmid\",\"method\":\"ga\",\"status\":\"detected\",\"evidence\":\"$found\"}"
    else
        echo "{\"vmid\":\"$vmid\",\"method\":\"ga\",\"status\":\"clean\"}"
    fi
}

# 磁盘挂载方式检测
detect_disk() {
    local vmid="$1"
    local disk_path="/data/images/${vmid}/vm-${vmid}-disk-0.qcow2"

    if [ ! -f "$disk_path" ]; then
        echo "{\"vmid\":\"$vmid\",\"method\":\"disk\",\"status\":\"error\",\"evidence\":\"no_disk\"}"
        return
    fi

    local temp_dir="/data/images/${TEMP_VMID}"
    local temp_disk="${temp_dir}/vm-${TEMP_VMID}-disk-0.qcow2"

    mkdir -p "$temp_dir"
    if ! cp "$disk_path" "$temp_disk" 2>/dev/null; then
        echo "{\"vmid\":\"$vmid\",\"method\":\"disk\",\"status\":\"error\",\"evidence\":\"copy_failed\"}"
        rm -rf "$temp_dir"
        return
    fi

    modprobe nbd max_part=8 2>/dev/null || true
    if ! timeout 15 qemu-nbd --read-only --connect=/dev/nbd0 "$temp_disk" 2>/dev/null; then
        echo "{\"vmid\":\"$vmid\",\"method\":\"disk\",\"status\":\"error\",\"evidence\":\"nbd_failed\"}"
        rm -rf "$temp_dir"
        return
    fi

    mkdir -p "$MOUNT_POINT"
    local mounted=0
    for part in nbd0p5 nbd0p2 nbd0p1; do
        if mount -o ro "/dev/$part" "$MOUNT_POINT" 2>/dev/null; then
            mounted=1
            break
        fi
    done

    if [ $mounted -eq 1 ]; then
        local found
        found=$(check_evidence "$MOUNT_POINT")
        umount "$MOUNT_POINT" 2>/dev/null

        if [ -n "$found" ]; then
            echo "{\"vmid\":\"$vmid\",\"method\":\"disk\",\"status\":\"detected\",\"evidence\":\"$found\"}"
        else
            echo "{\"vmid\":\"$vmid\",\"method\":\"disk\",\"status\":\"clean\"}"
        fi
    else
        echo "{\"vmid\":\"$vmid\",\"method\":\"disk\",\"status\":\"error\",\"evidence\":\"mount_failed\"}"
    fi

    qemu-nbd --disconnect /dev/nbd0 2>/dev/null || true
    rm -rf "$temp_dir"
}

# 检测单个 VM（仅 GA 模式，1秒超时）
detect_vm() {
    local vmid="$1"
    if timeout 1 qm guest cmd "$vmid" ping &>/dev/null 2>&1; then
        detect_ga "$vmid"
    else
        # GA 不可用，跳过（磁盘扫描太慢不适合批量）
        echo "{\"vmid\":\"$vmid\",\"method\":\"ga\",\"status\":\"skipped\",\"evidence\":\"no_guest_agent\"}"
    fi
}

# 主逻辑
echo '{"results":['
first=true

if [ "$MODE" = "single" ] && [ -n "$TARGET_VMID" ]; then
    result=$(detect_vm "$TARGET_VMID")
    echo "$result"
else
    # 获取运行中的VM列表（10秒超时）
    vm_list=$(timeout 10 qm list 2>/dev/null | awk 'NR>1 && $3=="running"{print $1}')
    for vmid in $vm_list; do
        if [ "$first" = true ]; then
            first=false
        else
            echo ","
        fi
        detect_vm "$vmid"
    done
fi

echo ']}'
