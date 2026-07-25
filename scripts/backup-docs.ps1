<#
.SYNOPSIS
    voice-ime 协作文档定期备份（Gavin 2026-07-25 指令）

.DESCRIPTION
    背景：.gitignore 排除了 /collab、/logs、/tasks、CLAUDE.md 等文档目录，
    它们不受 git 管辖——好处是 07-24 的 git 事故（troubleshooting
    [GIT-RESET-INCIDENT-001]）没能伤到它们，坏处是零版本历史、零备份，
    一次误删或覆盖即永久丢失。本脚本提供独立于 git 的快照备份。

    每次运行在 Backup/ 下创建 yyyyMMdd_HHmm 时间戳快照目录，
    默认保留最近 30 份，超出的最旧快照自动清理。
    快照为完整副本（非增量），单份约数百 KB，可直接翻阅。

.PARAMETER Keep
    保留的快照份数，默认 30。

.PARAMETER DryRun
    只打印将要执行的操作，不实际写入。

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\backup-docs.ps1
    powershell -ExecutionPolicy Bypass -File scripts\backup-docs.ps1 -Keep 60
#>
[CmdletBinding()]
param(
    [int]$Keep = 30,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

# 项目根 = 本脚本所在目录的上一级
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$BackupRoot  = Join-Path $ProjectRoot 'Backup'

# 备份清单：git 不管辖但不可丢失的文档资产
$Targets = @(
    @{ Path = 'collab';       Type = 'Dir'  },   # todo/progress/handoffs/decisions/troubleshooting/research 等
    @{ Path = 'logs';         Type = 'Dir'  },   # 每日改动日志（唯一副本）
    @{ Path = 'tasks';        Type = 'Dir'  },   # 任务与 lessons
    @{ Path = 'CHANGELOG.md'; Type = 'File' },   # 已跟踪，但 07-24 唯一阵亡者，一并备份
    @{ Path = 'CLAUDE.md';    Type = 'File' }    # 项目指引，gitignore 排除
)

# 排除项：
#   inbox/outbox/acks/drafts —— Worker 收发件箱与临时产物，无留存价值
#   audio-002B / audio-real-gavin —— PoC 原始录音 785 个 wav 共 62MB。
#     Gavin 2026-07-25 明确：不属于不可再生资产，后续需要可以重新录制，
#     无需备份也无需归档，丢失可接受。故永久排除，不要再"好心"加回来。
$ExcludeDirs = @('inbox', 'outbox', 'acks', 'drafts', 'audio-002B', 'audio-real-gavin')

$stamp    = Get-Date -Format 'yyyyMMdd_HHmm'
$destRoot = Join-Path $BackupRoot $stamp

Write-Host "[backup-docs] 项目根 : $ProjectRoot"
Write-Host "[backup-docs] 快照   : $destRoot"
if ($DryRun) { Write-Host "[backup-docs] *** DryRun 模式，不实际写入 ***" }

if (-not $DryRun) {
    if (-not (Test-Path $BackupRoot)) {
        New-Item -ItemType Directory -Path $BackupRoot -Force | Out-Null
    }
    if (Test-Path $destRoot) {
        Write-Host "[backup-docs] 同分钟快照已存在，跳过：$stamp"
        exit 0
    }
    New-Item -ItemType Directory -Path $destRoot -Force | Out-Null
}

$copiedFiles = 0
$copiedBytes = 0

foreach ($t in $Targets) {
    $src = Join-Path $ProjectRoot $t.Path
    if (-not (Test-Path $src)) {
        Write-Host ("  - {0,-14} 不存在，跳过" -f $t.Path)
        continue
    }

    if ($t.Type -eq 'File') {
        $size = (Get-Item $src).Length
        Write-Host ("  + {0,-14} {1} B" -f $t.Path, $size)
        if (-not $DryRun) {
            Copy-Item -LiteralPath $src -Destination (Join-Path $destRoot $t.Path) -Force
        }
        $copiedFiles++
        $copiedBytes += $size
        continue
    }

    # 目录：递归复制，跳过 $ExcludeDirs
    $files = Get-ChildItem -LiteralPath $src -Recurse -File | Where-Object {
        $rel = $_.FullName.Substring($src.Length).TrimStart('\')
        $parts = $rel -split '\\'
        -not ($parts | Where-Object { $ExcludeDirs -contains $_ })
    }

    $dirBytes = ($files | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $dirBytes) { $dirBytes = 0 }
    Write-Host ("  + {0,-14} {1} 个文件, {2} B" -f $t.Path, $files.Count, $dirBytes)

    if (-not $DryRun) {
        foreach ($f in $files) {
            $rel     = $f.FullName.Substring($ProjectRoot.Length).TrimStart('\')
            $target  = Join-Path $destRoot $rel
            $tgtDir  = Split-Path -Parent $target
            if (-not (Test-Path $tgtDir)) {
                New-Item -ItemType Directory -Path $tgtDir -Force | Out-Null
            }
            Copy-Item -LiteralPath $f.FullName -Destination $target -Force
        }
    }

    $copiedFiles += $files.Count
    $copiedBytes += $dirBytes
}

Write-Host ("[backup-docs] 本次快照 {0} 个文件, {1:N0} KB" -f $copiedFiles, ($copiedBytes / 1KB))

# ===== 保留策略：只留最近 $Keep 份 =====
if (-not $DryRun) {
    $snapshots = Get-ChildItem -LiteralPath $BackupRoot -Directory |
                 Where-Object { $_.Name -match '^\d{8}_\d{4}$' } |
                 Sort-Object Name -Descending

    if ($snapshots.Count -gt $Keep) {
        $stale = $snapshots | Select-Object -Skip $Keep
        foreach ($s in $stale) {
            Write-Host "[backup-docs] 清理旧快照：$($s.Name)"
            Remove-Item -LiteralPath $s.FullName -Recurse -Force -Confirm:$false
        }
    }
    Write-Host ("[backup-docs] 现存快照 {0} 份（保留上限 {1}）" -f
        [Math]::Min($snapshots.Count, $Keep), $Keep)
}

Write-Host "[backup-docs] 完成"
