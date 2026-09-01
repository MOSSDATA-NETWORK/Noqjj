#!/bin/bash
# chicken-check.sh — PVE 切鸡检测脚本 (v4)
# 用法:
#   chicken-check.sh --all                  批量扫描（并行，GA模式进VM检查，无GA标记needs_disk）
#   chicken-check.sh --vmid <id>            单台扫描（GA优先，无GA则磁盘挂载）
#   chicken-check.sh --vmid <id> --disk     强制磁盘挂载模式
#   chicken-check.sh --check-agent          检测脚本是否已安装
#
# 检测原理：
#   GA模式  : qm guest exec 进入VM检查 文件/systemd/bash_history/网络（1-3秒/台）
#   磁盘模式: 复制磁盘→qemu-nbd只读挂载→检查文件系统（10-60秒/台，仅单台）
# 检测特征：/opt/incus, incushlii-agent, nodeget-agent, incus/lxd服务, history关键词
# 输出：JSON

set -uo pipefail

SELF="$0"
MODE="all"
TARGET_VMID=""
VM_STATUS=""
FORCE_DISK=false
ONELINE=false
PARALLEL="${CHICKEN_PARALLEL:-20}"
GA_TIMEOUT=8
MOUNT_POINT="/tmp/chicken-mount-$$"

while [[ $# -gt 0 ]]; do
    case $1 in
        --vmid) TARGET_VMID="$2"; MODE="single"; shift 2 ;;
        --vmstatus) VM_STATUS="$2"; shift 2 ;;
        --all) MODE="all"; shift ;;
        --disk) FORCE_DISK=true; shift ;;
        --oneline) ONELINE=true; shift ;;
        --check-agent) echo '{"ok":true,"agent":"installed"}'; exit 0 ;;
        --version) echo "4"; exit 0 ;;
        *) shift ;;
    esac
done

# ---- VM 内检测逻辑（base64 编码后经 qm guest exec 传入，避免引号地狱）----
CHECK_B64=$(cat <<'CHECKEOF' | base64 | tr -d '\n'
found=""
[ -d /opt/incus ] && found="${found}incus_dir "
[ -e /usr/local/bin/incushlii-agent ] && found="${found}incushlii_agent "
[ -e /usr/local/bin/nodeget-agent ] && found="${found}nodeget_agent "
[ -d /var/lib/incus ] && found="${found}incus_data "
[ -d /var/lib/lxd ] && found="${found}lxd "
svc=$(ls /etc/systemd/system/ 2>/dev/null | grep -ciE "incus|shlii|nodeget" || true)
[ "${svc:-0}" -gt 0 ] && found="${found}svc:${svc} "
for h in /root/.bash_history /home/*/.bash_history; do
  [ -f "$h" ] || continue
  hc=$(grep -ciE "shlii\.io|incushlii|nodeget|nodehatch" "$h" 2>/dev/null || true)
  [ "${hc:-0}" -gt 0 ] && found="${found}hist:${hc} "
done
net=$(ss -tnp 2>/dev/null | grep -ciE "nodeget|incushlii|nodehatch" || true)
[ "${net:-0}" -gt 0 ] && found="${found}net:${net} "
[ -n "$found" ] && echo "FOUND:${found}" || echo "CLEAN"
CHECKEOF
)

# 从 qm guest exec 的 JSON 输出中提取 out-data 内容
extract_out() {
    # 输入: {"exitcode":0,"exited":1,"out-data":"CLEAN\n",...}
    grep -o '"out-data"[[:space:]]*:[[:space:]]*"[^"]*"' \
        | sed 's/.*:[[:space:]]*"//; s/"$//; s/\\n//g'
}

# GA 模式检测：进入 VM 内部
check_vm_ga() {
    local vmid="$1"
    local out
    out=$(timeout "$GA_TIMEOUT" qm guest exec "$vmid" -- bash -c "echo $CHECK_B64 | base64 -d | bash" 2>/dev/null | extract_out)
    if echo "$out" | grep -q "FOUND:"; then
        local ev
        ev=$(echo "$out" | sed 's/.*FOUND://; s/[[:space:]]*$//')
        echo "detected|ga|$ev"
    elif echo "$out" | grep -q "CLEAN"; then
        echo "clean|ga|"
    else
        echo "error|ga|exec_failed"
    fi
}

# 磁盘挂载检测：复制磁盘→qemu-nbd只读挂载→检查文件系统
check_vm_disk() {
    local vmid="$1"
    # 查找磁盘路径（支持 local-lvm 和 dir 存储）
    local disk_line disk_path
    disk_line=$(qm config "$vmid" 2>/dev/null | grep -E '^(scsi0|virtio0|sata0|ide0):' | head -1)
    if [ -z "$disk_line" ]; then
        echo "error|disk|no_disk_config"; return
    fi
    local disk_vol
    disk_vol=$(echo "$disk_line" | sed 's/^[^:]*: *//; s/,.*//')

    local nbd_dev="/dev/nbd1"
    modprobe nbd max_part=8 2>/dev/null || true
    qemu-nbd --disconnect "$nbd_dev" >/dev/null 2>&1 || true
    sleep 1

    # 解析实际磁盘文件路径
    local real_path
    real_path=$(pvesm path "$disk_vol" 2>/dev/null)
    if [ -z "$real_path" ]; then
        echo "error|disk|path_not_found"; return
    fi

    # LVM 块设备直接连，qcow2/raw 文件也可连
    if ! qemu-nbd --read-only --connect="$nbd_dev" "$real_path" 2>/dev/null; then
        echo "error|disk|nbd_failed"; return
    fi
    sleep 2

    mkdir -p "$MOUNT_POINT"
    local mounted=0 part
    for part in nbd1p5 nbd1p2 nbd1p1 nbd1p3 nbd1; do
        if mount -o ro,noload "/dev/$part" "$MOUNT_POINT" 2>/dev/null; then
            mounted=1; break
        fi
    done

    local result="error|disk|mount_failed"
    if [ $mounted -eq 1 ]; then
        local found=""
        [ -d "$MOUNT_POINT/opt/incus" ] && found="${found}incus_dir "
        [ -e "$MOUNT_POINT/usr/local/bin/incushlii-agent" ] && found="${found}incushlii_agent "
        [ -e "$MOUNT_POINT/usr/local/bin/nodeget-agent" ] && found="${found}nodeget_agent "
        [ -d "$MOUNT_POINT/var/lib/incus" ] && found="${found}incus_data "
        [ -d "$MOUNT_POINT/var/lib/lxd" ] && found="${found}lxd "
        local svc
        svc=$(ls "$MOUNT_POINT/etc/systemd/system/" 2>/dev/null | grep -ciE "incus|shlii|nodeget" || true)
        [ "${svc:-0}" -gt 0 ] && found="${found}svc:${svc} "
        local hc
        hc=$(grep -ciE "shlii\.io|incushlii|nodeget|nodehatch" "$MOUNT_POINT/root/.bash_history" 2>/dev/null || true)
        [ "${hc:-0}" -gt 0 ] && found="${found}hist:${hc} "
        umount "$MOUNT_POINT" 2>/dev/null
        if [ -n "$found" ]; then
            result="detected|disk|${found% }"
        else
            result="clean|disk|"
        fi
    fi

    qemu-nbd --disconnect "$nbd_dev" >/dev/null 2>&1 || true
    rmdir "$MOUNT_POINT" 2>/dev/null || true
    echo "$result"
}

# 检测单个 VM 并输出一行 JSON
detect_and_output() {
    local vmid="$1" vmstatus="$2"
    # 批量模式：stopped VM 跳过（磁盘扫描太慢，需手动触发）
    if [ "$MODE" = "all" ] && [ "$vmstatus" != "running" ]; then
        echo "{\"vmid\":\"$vmid\",\"method\":\"none\",\"status\":\"skipped\",\"evidence\":\"vm_stopped\"}"
        return
    fi

    local r
    if [ "$FORCE_DISK" = true ]; then
        r=$(check_vm_disk "$vmid")
    elif timeout 3 qm guest cmd "$vmid" ping &>/dev/null; then
        r=$(check_vm_ga "$vmid")
    elif [ "$MODE" = "single" ]; then
        # 单台模式无GA → 磁盘挂载
        r=$(check_vm_disk "$vmid")
    else
        # 批量模式无GA → 标记待磁盘扫描
        r="needs_disk_scan|none|no_guest_agent"
    fi

    local st m ev
    st=$(echo "$r" | cut -d'|' -f1)
    m=$(echo "$r" | cut -d'|' -f2)
    ev=$(echo "$r" | cut -d'|' -f3)
    echo "{\"vmid\":\"$vmid\",\"method\":\"$m\",\"status\":\"$st\",\"evidence\":\"$ev\"}"
}

# ---- oneline 模式（供 xargs 并行调用）----
if [ "$ONELINE" = true ]; then
    detect_and_output "$TARGET_VMID" "$VM_STATUS"
    exit 0
fi

# ---- 单台模式 ----
if [ "$MODE" = "single" ] && [ -n "$TARGET_VMID" ]; then
    echo '{"results":['
    detect_and_output "$TARGET_VMID" "running"
    echo ']}'
    exit 0
fi

# ---- 批量模式：xargs 并行自调用 ----
echo '{"results":['
qm list 2>/dev/null | awk 'NR>1 && $1 ~ /^[0-9]+$/ {print $1"|"$(NF-3)}' \
    | xargs -P "$PARALLEL" -I LINE bash -c 'v="${1%%|*}"; s="${1##*|}"; exec "'"$SELF"'" --oneline --vmid "$v" --vmstatus "$s"' _ LINE \
    | paste -sd',' -
echo ']}'
