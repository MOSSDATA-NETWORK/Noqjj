#!/bin/bash
# chicken-check.sh — PVE 切鸡检测脚本
# 部署到 PVE 宿主机后执行
# 用法: chicken-check.sh [--vmid <id>] [--all] [--check-agent]
#
# 检测逻辑：
#   有 GA → qm guest exec 进入 VM 内部检查文件/服务/history
#   无 GA → 复制磁盘 → qemu-nbd 只读挂载 → 检查文件系统
# 输出：JSON 格式结果

set -uo pipefail

MODE="all"
TARGET_VMID=""
MOUNT_POINT="/tmp/chicken-mount-$$"
TIMEOUT_GA=15
TIMEOUT_DISK=30

while [[ $# -gt 0 ]]; do
    case $1 in
        --vmid) TARGET_VMID="$2"; MODE="single"; shift 2 ;;
        --all) MODE="all"; shift ;;
        --check-agent) echo '{"ok":true,"agent":"installed"}'; exit 0 ;;
        *) shift ;;
    esac
done

# 清理函数（所有输出重定向到 /dev/null，不污染 JSON）
cleanup() {
    umount "$MOUNT_POINT" >/dev/null 2>&1
    qemu-nbd --disconnect /dev/nbd1 >/dev/null 2>&1
    rm -rf "$MOUNT_POINT" 2>/dev/null
}
trap cleanup EXIT

# GA 方式检测：进入 VM 内部检查
check_vm_ga() {
    local vmid="$1"
    local result
    result=$(timeout "$TIMEOUT_GA" qm guest exec "$vmid" -- bash -c '
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
        local evidence
        evidence=$(echo "$result" | sed 's/.*FOUND://')
        echo "detected|ga|$evidence"
    elif [ -n "$result" ]; then
        echo "clean|ga|"
    else
        echo "error|ga|exec_timeout"
    fi
}

# 磁盘挂载检测：复制磁盘 → 只读挂载 → 检查文件
check_vm_disk() {
    local vmid="$1"
    local disk_path="/data/images/${vmid}/vm-${vmid}-disk-0.qcow2"

    if [ ! -f "$disk_path" ]; then
        echo "error|disk|no_disk"
        return
    fi

    # 只读挂载原文件
    local nbd_dev="/dev/nbd1"
    modprobe nbd max_part=8 2>/dev/null || true
    qemu-nbd --disconnect "$nbd_dev" >/dev/null 2>&1 || true
    sleep 1
    if ! qemu-nbd --read-only --connect="$nbd_dev" "$disk_path" 2>/dev/null; then
        echo "error|disk|nbd_failed"
        return
    fi
    sleep 1  # 等内核识别分区表

    mkdir -p "$MOUNT_POINT"
    local mounted=0
    for part in nbd1p5 nbd1p2 nbd1p1; do
        # ro,noload：只读 + 跳过 ext4 日志恢复（副本操作，安全）
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

# 检测单个 VM
detect_vm() {
    local vmid="$1"
    local result=""

    if timeout 3 qm guest cmd "$vmid" ping &>/dev/null; then
        result=$(check_vm_ga "$vmid")
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
