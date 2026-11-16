param([string]$WorkspaceRoot = '.understand-book/beethoven9-tennstedt', [int]$FirstPort = 8891)
$ErrorActionPreference = 'Stop'
$repositoryRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$workspace = (Resolve-Path -LiteralPath (Join-Path $repositoryRoot $WorkspaceRoot)).Path
$executable = Join-Path $repositoryRoot 'target/debug/server.exe'
$publications = Get-Content -LiteralPath (Join-Path $workspace 'local-publications.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$running = @()
for ($index = 0; $index -lt $publications.Count; $index++) {
    $publication = $publications[$index].reference
    $directory = Join-Path $workspace "local-library/library/published/$($publication.book_id)/$($publication.publication_id)"
    if (-not (Test-Path -LiteralPath (Join-Path $directory 'publication.json'))) { throw "Local publication is missing: $($publication.book_id)" }
    $port = $FirstPort + $index
    $env:UNDERSTAND_BOOK_ADDR = "127.0.0.1:$port"
    $env:UNDERSTAND_BOOK_MEMORY_DIR = Join-Path $workspace "reader-memory/m$($index + 1)"
    $sessionPath = Join-Path $env:UNDERSTAND_BOOK_MEMORY_DIR 'session.json'
    if (Test-Path -LiteralPath $sessionPath) {
        $session = Get-Content -LiteralPath $sessionPath -Raw -Encoding UTF8 | ConvertFrom-Json
        $session.current_book_dir = $directory
        [System.IO.File]::WriteAllText($sessionPath, ($session | ConvertTo-Json -Depth 30), [System.Text.UTF8Encoding]::new($false))
    }
    $env:UNDERSTAND_BOOK_WEB_DIST = Join-Path $repositoryRoot 'packages/web/dist'
    $process = Start-Process -FilePath $executable -ArgumentList @('--reader-only', ('"' + $directory + '"')) -WorkingDirectory $repositoryRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $workspace "verification/server-m$($index + 1).log") -RedirectStandardError (Join-Path $workspace "verification/server-m$($index + 1).stderr.log")
    $running += [pscustomobject]@{ movement = "m$($index + 1)"; process_id = $process.Id; url = "http://127.0.0.1:$port/"; publication = $publication }
}
[System.IO.File]::WriteAllText((Join-Path $workspace 'local-readers.json'), ($running | ConvertTo-Json -Depth 4), [System.Text.UTF8Encoding]::new($false))
$running | ConvertTo-Json -Depth 4
