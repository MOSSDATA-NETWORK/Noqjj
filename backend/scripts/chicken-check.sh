#!/bin/bash
# chicken-check.sh — PVE 切鸡检测脚本
# 用法:
#   chicken-check.sh --all              # 批量扫描（仅GA模式，快速）
#   chicken-check.sh --vmid <id>        # 单台扫描（GA优先，GA不可用则磁盘挂载）
#   chicken-check.sh --vmid <id> --disk # 强制磁盘挂载模式
#   chicken-check.sh --check-agent      # 检测脚本是否已安装
#
# 检测逻辑：
#   GA模式：qm guest exec 进入VM内部检查文件/服务/history（快速，3-5秒/台）
#   磁盘模式：qemu-nbd只读挂载检查文件系统（慢，10-15秒/台，仅单台使用）
# 输出：JSON格式结果

set -uo pipefail

MODE="all"
TARGET_VMID=""
FORCE_DISK=false
MOUNT_POINT="/tmp/chicken-mount-$$"
GA_TIMEOUT=5

while [[ $# -gt 0 ]]; do
    case $1 in
        --vmid) TARGET_VMID="$2"; MODE="single"; shift 2 ;;
        --all) MODE="all"; shift ;;
        --disk) FORCE_DISK=true; shift ;;
        --check-agent) echo '{"ok":true,"agent":"installed"}'; exit 0 ;;
        *) shift ;;
    esac
done

cleanup() {
    umount "$MOUNT_POINT" >/dev/null 2>&1
    qemu-nbd --disconnect /dev/nbd1 >/dev/null 2>&1
    rm -rf "$MOUNT_POINT" 2>/dev/null
}
trap cleanup EXIT

# GA模式：进入VM内部检查
check_vm_ga() {
    local vmid="$1"
    local result
    result=$(timeout "$GA_TIMEOUT" qm guest exec "$vmid" -- bash -c '
        found=""
        [ -d /opt/incus ] && found="${found}incus_dir "
        [ -f /usr/local/bin/incushlii-agent ] && found="${found}incushlii_agent "
        [ -f /usr/local/bin/nodeget-agent ] && found="${found}nodeget_agent "
        [ -d /var/lib/incus ] && found="${found}incus_data "
        [ -d /var/lib/lxd ] && found="${found}lxd "
        svc=$(ls /etc/systemd/system/ 2>/dev/null | grep -ciE "incus|shlii|nodeget" || true)
        [ "$svc" -gt 0 ] 2>/dev/null && found="${found}systemd_svc:${svc} "
        hist=$(grep -ciE "shlii\.io|incushlii|nodeget|nodehatch" /root/.bash_history 2>/dev/null || true)
        [ "$hist" -gt 0 ] 2>/dev/null && found="${found}history:${hist} "
        net=$(ss -tnp 2>/dev/null | grep -ciE "nodeget|incushlii|ji\.778822|nodehatch" || true)
        [ "$net" -gt 0 ] 2>/dev/null && found="${found}network:${net} "
        [ -n "$found" ] && echo "FOUND:$found" || echo "CLEAN"
    ' 2>/dev/null | tr -d '\r\n\t ')

    if echo "$result" | grep -q "FOUND:"; then
        echo "detected|ga|$(echo "$result" | sed 's/.*FOUND://')"
    elif [ -n "$result" ]; then
        echo "clean|ga|"
    else
        echo "error|ga|exec_timeout"
    fi
}

# 磁盘挂载模式：只读挂载检查文件系统
check_vm_disk() {
    local vmid="$1"
    local disk_path="/data/images/${vmid}/vm-${vmid}-disk-0.qcow2"

    if [ ! -f "$disk_path" ]; then
        echo "error|disk|no_disk"
        return
    fi

    local nbd_dev="/dev/nbd1"
    modprobe nbd max_part=8 2>/dev/null || true
    qemu-nbd --disconnect "$nbd_dev" >/dev/null 2>&1 || true
    sleep 1
    if ! qemu-nbd --read-only --connect="$nbd_dev" "$disk_path" 2>/dev/null; then
        echo "error|disk|nbd_failed"
        return
    fi
    sleep 1

    mkdir -p "$MOUNT_POINT"
    local mounted=0
    for part in nbd1p5 nbd1p2 nbd1p1; do
        if mount -o ro,noload "/dev/$part" "$MOUNT_POINT" 2>/dev/null; then
            mounted=1
            break
        fi
    done

    if [ $mounted -eq 1 ]; then
        local found=""
        [ -d "$MOUNT_POINT/opt/incus" ] && found="${found}incus_dir "
        [ -f "$MOUNT_POINT/usr/local/bin/incushlii-agent" ] && found="${found}incushlii_agent "
        [ -f "$MOUNT_POINT/usr/local/bin/nodeget-agent" ] && found="${found}nodeget_agent "
        [ -d "$MOUNT_POINT/var/lib/incus" ] && found="${found}incus_data "
        [ -d "$MOUNT_POINT/var/lib/lxd" ] && found="${found}lxd "
        local svc
        svc=$(ls "$MOUNT_POINT/etc/systemd/system/" 2>/dev/null | grep -ciE "incus|shlii|nodeget" || true)
        [ "$svc" -gt 0 ] 2>/dev/null && found="${found}systemd_svc:${svc} "
        local hist
        hist=$(grep -ciE "shlii\.io|incushlii|nodeget|nodehatch" "$MOUNT_POINT/root/.bash_history" 2>/dev/null || true)
        [ "$hist" -gt 0 ] 2>/dev/null && found="${found}history:${hist} "

        umount "$MOUNT_POINT" >/dev/null 2>&1

        if [ -n "$found" ]; then
            echo "detected|disk|$found"
        else
            echo "clean|disk|"
        fi
    else
        echo "error|disk|mount_failed"
    fi

    qemu-nbd --disconnect /dev/nbd1 >/dev/null 2>&1 || true
}

# 检测单个VM
detect_vm() {
    local vmid="$1"
    local result=""

    # 强制磁盘模式
    if [ "$FORCE_DISK" = true ]; then
        result=$(check_vm_disk "$vmid")
    # GA可用 → GA模式
    elif timeout 2 qm guest cmd "$vmid" ping &>/dev/null; then
        result=$(check_vm_ga "$vmid")
    # 批量模式：无GA跳过（太慢）
    elif [ "$MODE" = "all" ]; then
        result="skipped|none|no_ga"
    # 单台模式：无GA → 磁盘挂载
    else
        result=$(check_vm_disk "$vmid")
    fi

    local status=$(echo "$result" | cut -d'|' -f1)
    local method=$(echo "$result" | cut -d'|' -f2)
    local evidence=$(echo "$result" | cut -d'|' -f3)

    echo "{\"vmid\":\"$vmid\",\"method\":\"$method\",\"status\":\"$status\",\"evidence\":\"$evidence\"}"
}

# 主逻辑
echo '{"results":['
first=true

if [ "$MODE" = "single" ] && [ -n "$TARGET_VMID" ]; then
    detect_vm "$TARGET_VMID"
else
    while IFS= read -r line; do
        vmid=$(echo "$line" | awk '{print $1}')
        [ -z "$vmid" ] && continue

        if [ "$first" = true ]; then
            first=false
        else
            echo ","
        fi

        detect_vm "$vmid"
    done < <(qm list 2>/dev/null | tail -n +2)
fi

echo ']}'
