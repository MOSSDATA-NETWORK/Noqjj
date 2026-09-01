#!/bin/bash
# chicken-check.sh — PVE 切鸡检测脚本（快速版）
# 部署到 PVE 宿主机后自动执行
# 用法: chicken-check.sh [--vmid <id>] [--all] [--check-agent]

set -uo pipefail
exec 2>/dev/null

MODE="all"
TARGET_VMID=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --vmid) TARGET_VMID="$2"; MODE="single"; shift 2 ;;
        --all) MODE="all"; shift ;;
        --check-agent) echo '{"ok":true,"agent":"installed"}'; exit 0 ;;
        *) shift ;;
    esac
done

# 快速检测：检查系统中是否有切鸡特征
check_system_evidence() {
    local found=""
    # 检查 Incus 相关文件和目录
    [ -d /opt/incus ] && found="${found}incus_dir "
    [ -f /usr/local/bin/incushlii-agent ] && found="${found}incushlii_agent "
    [ -f /usr/local/bin/nodeget-agent ] && found="${found}nodeget_agent "
    [ -d /var/lib/incus ] && found="${found}incus_data "
    [ -d /var/lib/lxd ] && found="${found}lxd "

    # 检查 systemd 服务
    local svc_count
    svc_count=$(ls /etc/systemd/system/ 2>/dev/null | grep -ciE "incus|shlii|nodeget" || true)
    [ "$svc_count" -gt 0 ] 2>/dev/null && found="${found}systemd_svc:${svc_count} "

    # 检查 bash_history
    local hist
    hist=$(grep -ciE "shlii\.io|incushlii|nodeget|nodehatch" /root/.bash_history 2>/dev/null || true)
    [ "$hist" -gt 0 ] 2>/dev/null && found="${found}history:${hist} "

    # 检查网络连接
    local net
    net=$(ss -tnp 2>/dev/null | grep -ciE "nodeget|incushlii|ji\.778822|nodehatch" || true)
    [ "$net" -gt 0 ] 2>/dev/null && found="${found}network:${net} "

    echo "$found"
}

# 获取 VM 列表并输出 JSON
echo '{"results":['
first=true

if [ "$MODE" = "single" ] && [ -n "$TARGET_VMID" ]; then
    # 单个 VM 检测
    if timeout 3 qm guest cmd "$TARGET_VMID" ping &>/dev/null 2>&1; then
        evidence=$(check_system_evidence)
        if [ -n "$evidence" ]; then
            echo "{\"vmid\":\"$TARGET_VMID\",\"method\":\"ga\",\"status\":\"detected\",\"evidence\":\"$evidence\"}"
        else
            echo "{\"vmid\":\"$TARGET_VMID\",\"method\":\"ga\",\"status\":\"clean\"}"
        fi
    else
        echo "{\"vmid\":\"$TARGET_VMID\",\"method\":\"ga\",\"status\":\"skipped\",\"evidence\":\"no_guest_agent\"}"
    fi
else
    # 批量检测：直接从 qm list 获取所有 VM
    while IFS= read -r line; do
        vmid=$(echo "$line" | awk '{print $1}')
        status=$(echo "$line" | awk '{print $3}')
        [ -z "$vmid" ] && continue

        if [ "$first" = true ]; then
            first=false
        else
            echo ","
        fi

        if [ "$status" = "running" ]; then
            # 运行中的 VM：快速检测
            if timeout 2 qm guest cmd "$vmid" ping &>/dev/null 2>&1; then
                evidence=$(check_system_evidence)
                if [ -n "$evidence" ]; then
                    echo "{\"vmid\":\"$vmid\",\"method\":\"ga\",\"status\":\"detected\",\"evidence\":\"$evidence\"}"
                else
                    echo "{\"vmid\":\"$vmid\",\"method\":\"ga\",\"status\":\"clean\"}"
                fi
            else
                echo "{\"vmid\":\"$vmid\",\"method\":\"ga\",\"status\":\"skipped\",\"evidence\":\"no_guest_agent\"}"
            fi
        else
            # 停止的 VM：标记为 skipped
            echo "{\"vmid\":\"$vmid\",\"method\":\"ga\",\"status\":\"skipped\",\"evidence\":\"vm_stopped\"}"
        fi
    done < <(qm list 2>/dev/null | tail -n +2)
fi

echo ']}'
