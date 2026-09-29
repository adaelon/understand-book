param(
    [string]$Python = "$PSScriptRoot/../../../tmp/ex9a-manim-venv/Scripts/python.exe",
    [ValidateSet('draft','final')][string]$Quality = 'final'
)
$ErrorActionPreference = 'Stop'
$Python = (Resolve-Path -LiteralPath $Python).Path
$taskTemp = Join-Path $PSScriptRoot '../../../tmp/ex9a-os-temp'
New-Item -ItemType Directory -Force $taskTemp | Out-Null
$env:TEMP = (Resolve-Path -LiteralPath $taskTemp).Path
$env:TMP = $env:TEMP
$env:PYTHONIOENCODING = 'utf-8'
Push-Location $PSScriptRoot
try {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $runName = "$Quality-$stamp"
    New-Item -ItemType Directory -Force 'runs' | Out-Null
    $env:EX9A_EVIDENCE = "runs/$runName-scene.json"
    $resolution = if ($Quality -eq 'final') { '1920,1080' } else { '854,480' }
    $fps = if ($Quality -eq 'final') { '30' } else { '15' }
    $timer = [Diagnostics.Stopwatch]::StartNew()
    & $Python -m manim --renderer=cairo --disable_caching --progress_bar none -r $resolution --fps $fps --media_dir "media/$runName" -o learning-rate scene.py LearningRate *> "runs/$runName.log"
    $renderExit = $LASTEXITCODE
    $timer.Stop()
    [ordered]@{run=$runName; seconds=$timer.Elapsed.TotalSeconds; exit=$renderExit; resolution=$resolution; fps=$fps} | ConvertTo-Json | Set-Content -Encoding utf8 "runs/$runName-cost.json"
    Get-Content "runs/$runName.log" -Tail 15
    if ($renderExit -ne 0) { throw "Render failed; retained runs/$runName.log" }
    Copy-Item -LiteralPath $env:EX9A_EVIDENCE -Destination scene-evidence.json
    $movie = Get-ChildItem -LiteralPath "media/$runName/videos/scene" -Recurse -Filter learning-rate.mp4 | Select-Object -First 1
    Copy-Item -LiteralPath $movie.FullName -Destination "learning-rate-$Quality.mp4"
    Get-Content "runs/$runName-cost.json"
} finally {
    Pop-Location
}
