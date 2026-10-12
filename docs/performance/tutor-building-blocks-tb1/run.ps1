param(
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [string]$ProviderEnv = 'E:/allwork/download/agent/understand-book/.env',
    [string]$Case = '',
    [string]$PriorOutput = ''
)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$outputPath = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $outputPath) { throw 'Use a fresh output directory; previous results are preserved.' }
# Load only the original Provider fields, without printing secrets or copying .env.
foreach ($line in [IO.File]::ReadAllLines($ProviderEnv)) {
    if ($line -match '^\s*(OPENCODE_API_KEY|OPENCODE_BASE_URL)\s*=\s*(.*?)\s*$') {
        $value = $Matches[2].Trim().Trim('"').Trim("'")
        [Environment]::SetEnvironmentVariable($Matches[1], $value, 'Process')
    }
}
$env:UNDERSTAND_BOOK_PROVIDER = 'native'
$env:FLUID_LLM_MODEL = 'deepseek-v4-flash'
$env:UB_OBSERVABILITY_MODE = 'off'
$env:CARGO_TARGET_DIR = 'D:/codex-build/understand-book-tb-195'
$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:CARGO_PROFILE_TEST_DEBUG = '0'
$env:CARGO_INCREMENTAL = '0'
$env:TEMP = 'D:/codex-build/tb1-temp'
$env:TMP = $env:TEMP
$env:TB1_OUTPUT = $outputPath
$env:TB1_CASE = $Case
$env:TB1_PRIOR_OUTPUT = $PriorOutput
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
Push-Location $repo
try {
    cargo +1.95.0 test -p server --lib -j 2 tests::tutor_building_blocks_tests::tutor_tb1_live_fixed_baseline -- --ignored --exact --nocapture *> "$outputPath.log"
    $exitCode = $LASTEXITCODE
    Get-Content -LiteralPath "$outputPath.log" -Tail 16
    exit $exitCode
} finally { Pop-Location }
