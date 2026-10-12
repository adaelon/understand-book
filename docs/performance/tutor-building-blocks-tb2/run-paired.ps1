param(
    [Parameter(Mandatory=$true)][string]$OutputRoot,
    [string]$ProviderEnv = 'E:/allwork/download/agent/understand-book/.env'
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$source = Join-Path $repo 'crates/runtime/src/tutor.rs'
$original = [IO.File]::ReadAllBytes($source)
$destination = [IO.Path]::GetFullPath($OutputRoot)
$runner = Join-Path $repo 'docs/performance/tutor-building-blocks-tb1/run.ps1'
try {
    foreach ($variant in @('before', 'after')) {
        $snapshot = if ($variant -eq 'before') { 'to3-baseline-tutor.rs' } else { 'to3-tb2-tutor.rs' }
        # Write the bytes with a fresh source timestamp so Cargo rebuilds this
        # variant; Copy-Item can retain a timestamp older than the cached build.
        [IO.File]::WriteAllBytes($source, [IO.File]::ReadAllBytes((Join-Path $PSScriptRoot $snapshot)))
        $output = Join-Path $destination "to3-$variant"
        & pwsh -NoProfile -File $runner -OutputDirectory $output -ProviderEnv $ProviderEnv
        if ($LASTEXITCODE -ne 0) { throw "Capture failed for $variant; see $output.log" }
        $configPath = Join-Path $output 'configuration.json'
        $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
        $config | Add-Member -NotePropertyName 'tb2_variant' -NotePropertyValue $variant
        $config | Add-Member -NotePropertyName 'tutor_source_snapshot' -NotePropertyValue $snapshot
        $config | Add-Member -NotePropertyName 'tool_baseline' -NotePropertyValue 'TO3: to3-baseline-tutor.rs and to3-integration.patch'
        $config | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath $configPath -Encoding utf8
        $budget = Get-Content -LiteralPath (Join-Path $output 'budget.json') -Raw | ConvertFrom-Json
        if ($budget.stopped) { throw 'Provider usage is unresolved; do not start another variant.' }
    }
} finally {
    [IO.File]::WriteAllBytes($source, $original)
}
